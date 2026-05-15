# Changelog

## 2025-01-30

### Changed
- Application now starts as a **background daemon** — no GUI window is shown on launch.
- A system tray icon appears in the OS status bar with a menu containing "Show Settings Panel" and "Exit".
- `DaemonApp` is the new invisible main `eframe::App` that manages tray, background threads, and viewports.
- Translator popup window is now a secondary viewport triggered by background mouse/translation threads.
- Settings panel is shown as a secondary viewport when "Show Settings Panel" is clicked in the tray menu.
- `trinity_translator::run()` is no longer called from `main.rs`; translation logic runs in daemon-managed background threads.

### Added
- `tray` module (`trinity-gui/src/tray/`) with platform-specific implementations:
  - macOS: NSStatusItem via `objc` + `cocoa` crates
  - Windows: Shell_NotifyIcon via `winapi` crate (message-only window in background thread)
  - Linux: StatusNotifierItem via `ksni` crate (D-Bus)
- `DaemonApp` (`trinity-gui/src/daemon.rs`) — invisible main eframe app that orchestrates tray and background tasks.
- `PanelApp::new_from_context()` and `PanelApp::show_inside()` methods in `trinity-panel` for daemon viewport integration.
- Basic settings panel UI with API URL, theme selection, hotkey placeholder, and about section.
- `PNG_BYTES` static in `trinity-util/src/icon.rs` for tray icon creation.
- Workspace dependencies: `objc`, `cocoa`, `winapi`, `ksni`, `rdev`, `cli-clipboard`, `deepl`.
- Workspace lints configuration (`unexpected_cfgs = "allow"` for `objc` crate compatibility).