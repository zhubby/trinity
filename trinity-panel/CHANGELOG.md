# Changelog

## 2026-05-15

### Added
- Dock-based Control Panel with fixed tabs for 通用, 快捷键, 剪切板, 翻译服务, and 语音服务.
- Basic settings persistence for API URL and theme.
- Editable hotkey settings for opening the translator, translating the selection, and quitting Trinity.
- Editable clipboard settings for history capacity, picker page size, and picker shortcut.
- Save flow that validates shortcuts, persists settings, and asks the daemon to reload hotkeys immediately.
- 通用 tab now uses `egui-theme-switch` for system/dark/light theme switching.

### Changed
- Control Panel now displays and saves the user-local JSON config path.
- Renamed the user-facing settings panel language to Control Panel.
- Theme changes are applied and saved immediately when the switch changes.

### Fixed
- Avoid a settings mutex self-deadlock when initializing the panel and loading hotkey configuration.

## 2025-01-30

### Added
- `PanelApp::new_from_context()` — creates a PanelApp from an egui `Context` (for daemon viewport usage).
- `PanelApp::show_inside()` — draws panel contents inside an existing `egui::Ui` (for immediate-mode viewports).
- Basic settings panel UI with sections:
  - Translation API URL configuration
  - Theme selection (dark/light)
  - Hotkey configuration placeholder
  - About section with version and GitHub link
- `log` workspace dependency.

### Changed
- `PanelApp` struct now has `api_url` and `theme` fields read from config.
- `eframe::App::ui()` implementation delegates to `show_inside()`.
