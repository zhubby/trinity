# Changelog

## 2026-05-15

### Added
- Editable hotkey settings for opening the translator, translating the selection, and quitting Trinity.
- Save flow that validates shortcuts, persists settings, and asks the daemon to reload hotkeys immediately.

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
