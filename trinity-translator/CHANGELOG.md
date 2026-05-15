# Changelog

## 2026-05-15

### Changed
- Removed translator-owned system hotkey registration; application-wide shortcuts now live in `trinity-util` and are orchestrated by `trinity`.

### Fixed
- Selection-copy helper no longer panics when clipboard initialization fails.
