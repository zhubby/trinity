//! Icon loading utilities for Trinity application windows and tray
//!
//! Provides access to embedded PNG bytes and egui `IconData`.
//!
//! Window icons use the full-color logo, while native tray/status icons use
//! a separate alpha-mask-friendly image for platform template rendering.

use egui::IconData;
use image::ImageFormat;

/// Raw PNG bytes of the Trinity logo, embedded at compile time.
pub static LOGO_PNG_BYTES: &[u8] = include_bytes!("../assets/logo.png");

/// Raw PNG bytes of the Trinity tray icon, embedded at compile time.
///
/// This image is intended for system tray/status-bar rendering. On macOS it is
/// used as a template image, so the alpha channel controls the shape while the
/// system supplies black/white colors for the current menu bar appearance.
pub static TRAY_PNG_BYTES: &[u8] = include_bytes!("../assets/tray.png");

/// Load window icon from embedded PNG resource as egui `IconData`.
///
/// Returns `None` if the PNG cannot be decoded.
#[must_use]
pub fn get_icon_data() -> Option<IconData> {
    let img = image::load_from_memory_with_format(LOGO_PNG_BYTES, ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(IconData {
        rgba: rgba.as_raw().clone(),
        width,
        height,
    })
}
