# Changelog

## 2026-05-15

### Added
- JSON-backed configuration persistence at `~/.trinity/config.json`.
- Basic config persistence helper for API URL and theme updates.
- Shared `hotkey` module built on `global-hotkey` for application-wide system shortcut registration.
- Hotkey config loading and targeted persistence helpers.
- Dedicated `TRAY_PNG_BYTES` resource for native system tray/status icons.

### Changed
- Configuration loading now uses the user-local Trinity JSON config instead of the legacy translator settings file.
- Resource files moved from `res/` to `assets/`.
- Window icons now use `LOGO_PNG_BYTES` while tray implementations use `TRAY_PNG_BYTES`.

## 2025-01-30

### Added
- `PNG_BYTES` static constant in `icon.rs` — exposes the raw logo PNG bytes for tray icon creation.
- `#[must_use]` attribute on `get_icon_data()`.
- Re-export of `PNG_BYTES` in `lib.rs`.
