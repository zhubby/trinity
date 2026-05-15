//! Windows tray implementation using Shell_NotifyIcon
//!
//! Creates a message-only window in a background thread that hosts
//! the tray icon and processes its WM_TRAYICON messages.
//! Tray events are forwarded to the main thread via `mpsc`.

use log::warn;
use std::sync::{LazyLock, Mutex, mpsc};
use winapi::{
    shared::{
        minwindef::{HIWORD, LPARAM, LRESULT, UINT, WPARAM},
        ntdef::LPCWSTR,
        windef::HWND,
    },
    um::{
        libloaderapi::GetModuleHandleW,
        shellapi::{
            NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
        },
        winuser::{
            CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
            HWND_MESSAGE, ICON_SMALL, PostQuitMessage, RegisterClassExW, SendMessageW, WM_DESTROY,
            WM_USER, WNDCLASSEXW, WS_EX_NOACTIVATE,
        },
    },
};

/// Events emitted by the tray menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    ShowPanel,
    Exit,
}

/// Custom message ID for tray icon notifications
const WM_TRAYICON: UINT = WM_USER + 1;

/// ID for the context menu "Show Settings Panel" item
const ID_SHOW_PANEL: UINT = 1001;
/// ID for the context menu "Exit" item
const ID_EXIT: UINT = 1002;

/// Global sender — the background thread writes tray events here.
static TRAY_TX: LazyLock<Mutex<Option<mpsc::Sender<TrayEvent>>>> =
    LazyLock::new(|| Mutex::new(None));

// ── Window procedure for the message-only tray window ────────────────

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TRAYICON && HIWORD(lparam as u32) as UINT == winapi::um::winuser::WM_RBUTTONUP {
        // Right-click on tray icon → show context menu
        show_context_menu(hwnd);
        return 0;
    }

    if msg == winapi::um::winuser::WM_COMMAND {
        let cmd_id = LOWORD(lparam as u32) as UINT;
        match cmd_id {
            ID_SHOW_PANEL => {
                let guard = TRAY_TX.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(sender) = guard.as_ref() {
                    sender.send(TrayEvent::ShowPanel).ok();
                }
                return 0;
            }
            ID_EXIT => {
                let guard = TRAY_TX.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(sender) = guard.as_ref() {
                    sender.send(TrayEvent::Exit).ok();
                }
                return 0;
            }
            _ => {}
        }
    }

    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn LOWORD(dw: u32) -> u16 {
    (dw & 0xFFFF) as u16
}

/// Show a popup context menu near the tray icon.
unsafe fn show_context_menu(hwnd: HWND) {
    use winapi::um::winuser::{
        AppendMenuW, CreatePopupMenu, GetCursorPos, MF_SEPARATOR, MF_STRING, POINT,
        SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
        TrackPopupMenu,
    };

    let hmenu = CreatePopupMenu();

    let mut panel_text: Vec<u16> = "Show Settings Panel\0".encode_utf16().collect();
    AppendMenuW(
        hmenu,
        MF_STRING,
        ID_SHOW_PANEL as WPARAM,
        panel_text.as_mut_ptr() as LPCWSTR,
    );

    AppendMenuW(hmenu, MF_SEPARATOR, 0, 0 as LPCWSTR);

    let mut exit_text: Vec<u16> = "Exit\0".encode_utf16().collect();
    AppendMenuW(
        hmenu,
        MF_STRING,
        ID_EXIT as WPARAM,
        exit_text.as_mut_ptr() as LPCWSTR,
    );

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);

    let cmd = TrackPopupMenu(
        hmenu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_NONOTIFY | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        0 as *mut _,
    );

    if cmd != 0 {
        SendMessageW(
            hwnd,
            winapi::um::winuser::WM_COMMAND,
            cmd as WPARAM,
            0 as LPARAM,
        );
    }

    winapi::um::winuser::DestroyMenu(hmenu);
}

// ── Public API ──────────────────────────────────────────────────────

/// Create the Windows system tray icon and menu.
///
/// Spawns a background thread that:
/// 1. Creates a message-only window
/// 2. Registers a Shell_NotifyIcon tray icon
/// 3. Runs a message loop for tray events
/// 4. Forward events to the returned `Receiver<TrayEvent>`
pub fn create_tray(_ctx: egui::Context, _tx: mpsc::Sender<TrayEvent>) -> mpsc::Receiver<TrayEvent> {
    let (tx, rx) = mpsc::channel();
    *TRAY_TX.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

    std::thread::spawn(move || {
        unsafe {
            let hinstance = GetModuleHandleW(0 as LPCWSTR);

            let mut class_name: Vec<u16> = "TrinityTrayWindow\0".encode_utf16().collect();

            let wnd_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as UINT,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: tray_window_proc,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: 0 as *mut _,
                hCursor: 0 as *mut _,
                hbrBackground: 0 as *mut _,
                lpszMenuName: 0 as LPCWSTR,
                lpszClassName: class_name.as_mut_ptr() as LPCWSTR,
                hIconSm: 0 as *mut _,
            };

            if RegisterClassExW(&wnd_class) == 0 {
                warn!("Failed to register tray window class");
                return;
            }

            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE,
                class_name.as_mut_ptr() as LPCWSTR,
                0 as LPCWSTR,
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE, // message-only window
                0 as *mut _,
                hinstance,
                0 as *mut _,
            );

            if hwnd.is_null() {
                warn!("Failed to create tray message window");
                return;
            }

            // Load icon from embedded PNG → HICON
            let hicon = load_icon();

            // Setup NOTIFYICONDATA
            let mut tip_text: Vec<u16> = "Trinity\0".encode_utf16().collect();
            let mut nid = NOTIFYICONDATAW {
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: WM_TRAYICON,
                hIcon: hicon,
                szTip: [0; 128],
                dwState: 0,
                dwStateMask: 0,
                uVersion: 0,
                guidItem: winapi::shared::guiddef::GUID {
                    Data1: 0,
                    Data2: 0,
                    Data3: 0,
                    Data4: [0; 8],
                },
                hBalloonIcon: 0 as *mut _,
            };

            // Copy tip text into szTip
            let tip_len = tip_text.len().min(128);
            for i in 0..tip_len {
                nid.szTip[i] = tip_text[i];
            }

            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                warn!("Shell_NotifyIconW NIM_ADD failed");
                return;
            }

            // Message loop
            let mut msg = winapi::um::winuser::MSG {
                hwnd: 0 as *mut _,
                message: 0,
                wParam: 0,
                lParam: 0,
                time: 0,
                pt: POINT { x: 0, y: 0 },
            };

            while GetMessageW(&mut msg, 0 as *mut _, 0, 0) != 0 {
                DispatchMessageW(&msg);
            }

            // Cleanup
            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            if !hicon.is_null() {
                winapi::um::winuser::DestroyIcon(hicon);
            }
        }
    });

    rx
}

/// Load the app icon from embedded PNG bytes as an HICON.
unsafe fn load_icon() -> winapi::shared::windef::HICON {
    use winapi::um::winuser::{CreateIconFromResourceEx, LR_DEFAULTCOLOR};

    let png_bytes = trinity_util::icon::TRAY_PNG_BYTES;
    // CreateIconFromResourceEx can load PNG icons on Windows Vista+
    CreateIconFromResourceEx(
        png_bytes.as_ptr() as *mut u8,
        png_bytes.len() as u32,
        1,                 // TRUE = icon
        0x00030000,        // version
        ICON_SMALL as u32, // or 0 for default size
        ICON_SMALL as u32,
        LR_DEFAULTCOLOR,
    )
}
