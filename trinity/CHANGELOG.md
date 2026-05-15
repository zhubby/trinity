# Changelog

## 2026-05-16

### Changed
- Package renamed from `trinity-gui` to `trinity` (directory `trinity/`).
- User-facing panel naming now uses Control Panel in the window title and tray menus.
- Settings and translator viewports now track close requests independently so each window can be closed and reopened without affecting the other.
- The settings panel is now the root viewport and is shown on startup.
- The Control Panel now uses a borderless egui menu bar with window action buttons, drag handling, and edge resize support.
- The system tray is created as soon as the eframe daemon starts instead of waiting for the Control Panel to be hidden.
- The daemon now applies the saved system/dark/light theme preference on startup.
- Window position persistence is disabled so previously hidden/off-screen panel positions do not affect startup visibility.
- macOS no longer switches the app to accessory activation policy when the settings panel is visible.
- Global hotkeys and mouse-listener startup are deferred until the settings panel is hidden, keeping the startup panel responsive.
- The root viewport is explicitly made visible from `App::logic()` to work around eframe's hidden-first-frame native startup behavior.
- macOS tray menu actions now wake the egui context before restoring the settings window from its hidden off-screen state.
- Translator popup remains a secondary viewport triggered by hotkeys or selection translation.

### Fixed
- Tray "Show Control Panel" now restores the hidden root viewport directly instead of trying to recover an off-screen parked window.
- macOS tray icon now uses the dedicated `tray.png` template asset, validates native image creation, and falls back to a text status item when the PNG cannot be decoded.
- macOS tray status item and menu delegate are retained for the lifetime of the app, preventing the menu bar icon from disappearing after startup.
- Settings and translator viewports are opened from the root `ui()` path again, avoiding freezes from creating child viewports inside `App::logic()`.
- Settings panel close requests now hide the root viewport instead of exiting the daemon.
- The settings panel close button now hides the root viewport instead of parking it off-screen or exiting the daemon.

### Added
- CLI argument `--log-level` via `clap` (default: `debug`). Supported levels: off, error, warn, info, debug, trace.
- `env_logger` integration for runtime log-level filtering.
- Clipboard history daemon integration with background polling and an always-on-top picker viewport.
- `CmdOrCtrl+Shift+V` global shortcut handling for opening clipboard history.

## 2026-05-15

### Added
- Application-wide system hotkey service in the daemon for opening the translator, translating the current selection, and quitting.
- Panel-to-daemon hotkey reload channel so saved shortcut changes take effect without restart.

## 2025-01-30

### Changed
- Application now starts as a **background daemon** — no GUI window is shown on launch.
- A system tray icon appears in the OS status bar with a menu containing "Show Settings Panel" and "Exit".
- `DaemonApp` is the new invisible main `eframe::App` that manages tray, background threads, and viewports.
- Translator popup window is now a secondary viewport triggered by background mouse/translation threads.
- Settings panel is shown as a secondary viewport when "Show Settings Panel" is clicked in the tray menu.
- `trinity_translator::run()` is no longer called from `main.rs`; translation logic runs in daemon-managed background threads.

### Added
- `tray` module (`trinity/src/tray/`) with platform-specific implementations:
  - macOS: NSStatusItem via `objc` + `cocoa` crates
  - Windows: Shell_NotifyIcon via `winapi` crate (message-only window in background thread)
  - Linux: StatusNotifierItem via `ksni` crate (D-Bus)
- `DaemonApp` (`trinity/src/daemon.rs`) — invisible main eframe app that orchestrates tray and background tasks.
- `PanelApp::new_from_context()` and `PanelApp::show_inside()` methods in `trinity-panel` for daemon viewport integration.
- Basic settings panel UI with API URL, theme selection, hotkey placeholder, and about section.
- `PNG_BYTES` static in `trinity-util/src/icon.rs` for tray icon creation.
- Workspace dependencies: `objc`, `cocoa`, `winapi`, `ksni`, `rdev`, `cli-clipboard`, `deepl`.
- Workspace lints configuration (`unexpected_cfgs = "allow"` for `objc` crate compatibility).
