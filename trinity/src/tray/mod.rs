//! System tray module — cross-platform status bar icon and menu
//!
//! Each platform implements `TrayEvent` and `create_tray()`:
//! - macOS: NSStatusItem via objc/cocoa
//! - Windows: Shell_NotifyIcon via winapi
//! - Linux: StatusNotifierItem via ksni (D-Bus)
//!
//! Tray menu items:
//! - "Show Control Panel" → sends TrayEvent::ShowPanel
//! - "Exit" → sends TrayEvent::Exit

cfg_if::cfg_if! {
    if #[cfg(target_os = "macos")] {
        #[allow(deprecated)] // cocoa crate is deprecated; planned migration to objc2
        mod macos;
        pub use macos::{TrayEvent, create_tray};
    } else if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::{TrayEvent, create_tray};
    } else {
        mod linux;
        pub use linux::{TrayEvent, create_tray};
    }
}
