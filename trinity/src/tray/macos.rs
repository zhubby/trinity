//! macOS tray implementation using NSStatusItem
//!
//! Creates a status bar icon with a dropdown menu containing:
//! - "Show Control Panel" → TrayEvent::ShowPanel
//! - "Exit" → TrayEvent::Exit
//!
//! This must be called AFTER eframe has initialized NSApplication,
//! so it's invoked after the daemon starts running its `logic()` loop.
//!
//! Note: The `cocoa` crate is deprecated in favor of `objc2-app-kit`.
//! We use `cocoa` for now as it's simpler and well-tested; migration
//! to `objc2` ecosystem is planned for a future release.

use cocoa::{
    appkit::{NSApp, NSMenu, NSStatusBar, NSVariableStatusItemLength},
    base::{id, nil},
    foundation::{NSSize, NSString},
};
use log::warn;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, YES},
    sel, sel_impl,
};
use std::{
    ffi::CString,
    sync::{LazyLock, Mutex, mpsc},
};

/// Events emitted by the tray menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowPanel,
    Exit,
}

/// Global channel for tray → daemon communication.
/// Tray menu actions write here; DaemonApp::logic() reads every frame.
static TRAY_CHANNEL: LazyLock<Mutex<Option<mpsc::Sender<TrayEvent>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Egui context used to wake the daemon when AppKit sends a tray event.
static REPAINT_CONTEXT: LazyLock<Mutex<Option<egui::Context>>> = LazyLock::new(|| Mutex::new(None));

/// Whether the tray has already been created (avoid duplicate status items)
static TRAY_CREATED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

/// Strong references to Objective-C tray objects that must outlive `create_tray`.
///
/// AppKit does not reliably keep the status item or menu target alive for us,
/// so dropping these local `id`s can make the menu bar icon disappear.
static TRAY_OBJECTS: LazyLock<Mutex<Option<TrayObjects>>> = LazyLock::new(|| Mutex::new(None));

struct TrayObjects {
    _status_item: usize,
    _delegate: usize,
}

// ── Objective-C delegate class ───────────────────────────────────────

/// Register a custom ObjC class `TrinityTrayDelegate` whose instance methods
/// map tray menu actions to `TrayEvent` writes on the global channel.
fn register_delegate_class() -> &'static Class {
    let superclass = class!(NSObject);
    let mut decl =
        ClassDecl::new("TrinityTrayDelegate", superclass).expect("class already registered");

    extern "C" fn show_panel(_: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
        log::info!("macOS tray Show Control Panel selected");
        let guard = TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = guard.as_ref() {
            sender.send(TrayEvent::ShowPanel).ok();
        }
        request_repaint();
    }

    extern "C" fn exit_app(_: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
        log::info!("macOS tray Exit selected");
        let guard = TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = guard.as_ref() {
            sender.send(TrayEvent::Exit).ok();
        }
        request_repaint();
        let app = unsafe { NSApp() };
        let _: () = unsafe { msg_send![app, terminate: nil] };
    }

    unsafe {
        decl.add_method(
            sel!(showPanel:),
            show_panel as extern "C" fn(&objc::runtime::Object, objc::runtime::Sel, id),
        );
        decl.add_method(
            sel!(exitApp:),
            exit_app as extern "C" fn(&objc::runtime::Object, objc::runtime::Sel, id),
        );
    }

    decl.register()
}

fn request_repaint() {
    let guard = REPAINT_CONTEXT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ctx) = guard.as_ref() {
        ctx.request_repaint();
    }
}

// ── Helper: convert Rust str → NSString ─────────────────────────────

/// Convert a Rust `&str` into a macOS `NSString` object.
///
/// Uses `CString` to ensure the string is null-terminated before
/// passing it to `initWithUTF8String:` — Rust `&str` is NOT
/// null-terminated, so passing `s.as_ptr()` directly causes
/// `strlen` inside `CFStringCreateWithCString` to read past the
/// string boundary, resulting in `EXC_BAD_ACCESS` / segfault.
fn ns_string(s: &str) -> id {
    let c_str = CString::new(s).unwrap_or_else(|_| CString::new("<invalid>").unwrap());
    unsafe {
        let ns_string: id = NSString::alloc(nil);
        msg_send![ns_string, initWithUTF8String: c_str.as_ptr()]
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Create the macOS system tray icon and menu.
///
/// Returns a `Receiver<TrayEvent>` so `DaemonApp` can poll tray events each frame.
/// Call this **once**, after eframe's NSApplication is running
/// (i.e. from the daemon after eframe has initialized).
pub fn create_tray(ctx: egui::Context, _tx: mpsc::Sender<TrayEvent>) -> mpsc::Receiver<TrayEvent> {
    let was_created = *TRAY_CREATED.lock().unwrap_or_else(|e| e.into_inner());
    if was_created {
        return mpsc::channel().1; // unused dummy
    }
    *TRAY_CREATED.lock().unwrap_or_else(|e| e.into_inner()) = true;
    *REPAINT_CONTEXT.lock().unwrap_or_else(|e| e.into_inner()) = Some(ctx);

    let (real_tx, rx) = mpsc::channel();
    *TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(real_tx);

    let delegate_class = register_delegate_class();
    let delegate: id = unsafe { msg_send![delegate_class, new] };

    // ── Status bar item ────────────────────────────────────────
    let status_bar: id = unsafe { NSStatusBar::systemStatusBar(nil) };
    let status_item: id =
        unsafe { msg_send![status_bar, statusItemWithLength: NSVariableStatusItemLength] };
    let _: () = unsafe { msg_send![status_item, setLength: 24.0] };
    let retained_status_item: id = unsafe { msg_send![status_item, retain] };
    let retained_delegate: id = unsafe { msg_send![delegate, retain] };

    // ── Icon ───────────────────────────────────────────────────
    let icon_bytes = trinity_util::icon::TRAY_PNG_BYTES;
    let nsdata: id = unsafe { msg_send![class!(NSData), alloc] };
    let nsdata: id =
        unsafe { msg_send![nsdata, initWithBytes: icon_bytes.as_ptr() length: icon_bytes.len()] };
    if nsdata == nil {
        warn!("failed to create NSData for macOS tray icon");
        set_status_item_title(status_item, "T");
    } else {
        let icon: id = unsafe { msg_send![class!(NSImage), alloc] };
        let icon: id = unsafe { msg_send![icon, initWithData: nsdata] };
        if icon == nil {
            warn!("failed to decode macOS tray icon PNG");
            set_status_item_title(status_item, "T");
        } else {
            let _: () = unsafe { msg_send![icon, setTemplate: YES] };
            let _: () = unsafe { msg_send![icon, setSize: NSSize::new(18.0, 18.0)] };
            set_status_item_image(status_item, icon);
        }
    }

    // ── Menu ───────────────────────────────────────────────────
    let menu: id = unsafe { NSMenu::new(nil) };
    let _: () = unsafe { msg_send![menu, setAutoenablesItems: false] };

    // "Show Control Panel"
    let panel_item: id = unsafe { msg_send![class!(NSMenuItem), alloc] };
    let panel_item: id = unsafe {
        msg_send![panel_item, initWithTitle: ns_string("Show Control Panel")
                                              action: sel!(showPanel:)
                                              keyEquivalent: ns_string("")]
    };
    let _: () = unsafe { msg_send![panel_item, setTarget: delegate] };

    // Separator
    let sep: id = unsafe { msg_send![class!(NSMenuItem), separatorItem] };

    // "Exit"
    let exit_item: id = unsafe { msg_send![class!(NSMenuItem), alloc] };
    let exit_item: id = unsafe {
        msg_send![exit_item, initWithTitle: ns_string("Exit")
                                            action: sel!(exitApp:)
                                            keyEquivalent: ns_string("")]
    };
    let _: () = unsafe { msg_send![exit_item, setTarget: delegate] };

    let _: () = unsafe { msg_send![menu, addItem: panel_item] };
    let _: () = unsafe { msg_send![menu, addItem: sep] };
    let _: () = unsafe { msg_send![menu, addItem: exit_item] };

    let _: () = unsafe { msg_send![status_item, setMenu: menu] };
    *TRAY_OBJECTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(TrayObjects {
        _status_item: retained_status_item as usize,
        _delegate: retained_delegate as usize,
    });

    rx
}

fn set_status_item_image(status_item: id, icon: id) {
    let button: id = unsafe { msg_send![status_item, button] };
    if button == nil {
        let _: () = unsafe { msg_send![status_item, setImage: icon] };
        return;
    }

    let _: () = unsafe { msg_send![button, setImage: icon] };
}

fn set_status_item_title(status_item: id, title: &str) {
    let title = ns_string(title);
    let button: id = unsafe { msg_send![status_item, button] };
    if button == nil {
        let _: () = unsafe { msg_send![status_item, setTitle: title] };
        return;
    }

    let _: () = unsafe { msg_send![button, setTitle: title] };
}
