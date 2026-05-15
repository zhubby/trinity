//! macOS tray implementation using NSStatusItem
//!
//! Creates a status bar icon with a dropdown menu containing:
//! - "Show Settings Panel" → TrayEvent::ShowPanel
//! - "Exit" → TrayEvent::Exit
//!
//! This must be called AFTER eframe has initialized NSApplication,
//! so it's invoked in the first `DaemonApp::ui()` call.
//!
//! Note: The `cocoa` crate is deprecated in favor of `objc2-app-kit`.
//! We use `cocoa` for now as it's simpler and well-tested; migration
//! to `objc2` ecosystem is planned for a future release.

use cocoa::{
    appkit::{NSMenu, NSStatusBar, NSVariableStatusItemLength},
    base::{id, nil},
    foundation::NSString,
};
use objc::{class, declare::ClassDecl, msg_send, runtime::Class, sel, sel_impl};
use std::sync::{LazyLock, Mutex, mpsc};

/// Events emitted by the tray menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowPanel,
    Exit,
}

/// Global channel for tray → daemon communication.
/// Tray menu actions write here; DaemonApp::ui() reads every frame.
static TRAY_CHANNEL: LazyLock<Mutex<Option<mpsc::Sender<TrayEvent>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Whether the tray has already been created (avoid duplicate status items)
static TRAY_CREATED: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

// ── Objective-C delegate class ───────────────────────────────────────

/// Register a custom ObjC class `TrinityTrayDelegate` whose instance methods
/// map tray menu actions to `TrayEvent` writes on the global channel.
fn register_delegate_class() -> &'static Class {
    let superclass = class!(NSObject);
    let mut decl =
        ClassDecl::new("TrinityTrayDelegate", superclass).expect("class already registered");

    extern "C" fn show_panel(_: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
        let guard = TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = guard.as_ref() {
            sender.send(TrayEvent::ShowPanel).ok();
        }
    }

    extern "C" fn exit_app(_: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
        let guard = TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = guard.as_ref() {
            sender.send(TrayEvent::Exit).ok();
        }
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

// ── Helper: convert Rust str → NSString ─────────────────────────────

fn ns_string(s: &str) -> id {
    unsafe {
        let ns_string: id = NSString::alloc(nil);
        msg_send![ns_string, initWithUTF8String: s.as_ptr() as *const i8]
    }
}

// ── Public API ──────────────────────────────────────────────────────

/// Create the macOS system tray icon and menu.
///
/// Returns a `Receiver<TrayEvent>` so `DaemonApp` can poll tray events each frame.
/// Call this **once**, after eframe's NSApplication is running
/// (i.e. inside `DaemonApp::ui()` on the first frame).
pub fn create_tray(_tx: mpsc::Sender<TrayEvent>) -> mpsc::Receiver<TrayEvent> {
    let was_created = *TRAY_CREATED.lock().unwrap_or_else(|e| e.into_inner());
    if was_created {
        return mpsc::channel().1; // unused dummy
    }
    *TRAY_CREATED.lock().unwrap_or_else(|e| e.into_inner()) = true;

    let (real_tx, rx) = mpsc::channel();
    *TRAY_CHANNEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(real_tx);

    let delegate_class = register_delegate_class();
    let delegate: id = unsafe { msg_send![delegate_class, new] };

    // ── Status bar item ────────────────────────────────────────
    let status_bar: id = unsafe { NSStatusBar::systemStatusBar(nil) };
    let status_item: id =
        unsafe { msg_send![status_bar, statusItemWithLength: NSVariableStatusItemLength] };

    // ── Icon ───────────────────────────────────────────────────
    let icon_bytes = trinity_util::icon::PNG_BYTES;
    let nsdata: id = unsafe { msg_send![class!(NSData), alloc] };
    let nsdata: id = unsafe {
        msg_send![nsdata, initWithBytes: icon_bytes.as_ptr() length: icon_bytes.len() as u64]
    };
    let icon: id = unsafe { msg_send![class!(NSImage), alloc] };
    let icon: id = unsafe { msg_send![icon, initWithData: nsdata] };
    let _: () = unsafe { msg_send![status_item, setImage: icon] };

    // ── Menu ───────────────────────────────────────────────────
    let menu: id = unsafe { NSMenu::new(nil) };
    let _: () = unsafe { msg_send![menu, setAutoenablesItems: false] };

    // "Show Settings Panel"
    let panel_item: id = unsafe { msg_send![class!(NSMenuItem), alloc] };
    let panel_item: id = unsafe {
        msg_send![panel_item, initWithTitle: ns_string("Show Settings Panel")
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

    rx
}
