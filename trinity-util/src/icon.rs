//! Icon loading utilities for Trinity application windows and tray
//!
//! Provides access to the embedded logo PNG bytes and egui `IconData`
//! for window icons. The raw PNG bytes (`PNG_BYTES`) are also exposed
//! so platform-specific tray code can create native icons from them.

use egui::IconData;
use image::ImageFormat;

/// Raw PNG bytes of the Trinity logo, embedded at compile time.
/// Available to all crates for creating native tray icons, window icons, etc.
pub static PNG_BYTES: &[u8] = include_bytes!("../res/logo.png");

/// Load window icon from embedded PNG resource as egui `IconData`.
///
/// Returns `None` if the PNG cannot be decoded.
#[must_use]
pub fn get_icon_data() -> Option<IconData> {
    let img = image::load_from_memory_with_format(PNG_BYTES, ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(IconData {
        rgba: rgba.as_raw().clone(),
        width,
        height,
    })
}
