# Changelog

## 2026-05-15

### Added
- Shared `hotkey` module built on `global-hotkey` for application-wide system shortcut registration.
- Hotkey config loading and targeted persistence helpers.

## 2025-01-30

### Added
- `PNG_BYTES` static constant in `icon.rs` — exposes the raw logo PNG bytes for tray icon creation.
- `#[must_use]` attribute on `get_icon_data()`.
- Re-export of `PNG_BYTES` in `lib.rs`.
