use egui::IconData;
use image::ImageFormat;

/// Load window icon from embedded PNG resource
pub fn get_icon_data() -> Option<IconData> {
    let png_bytes = include_bytes!("../res/logo.png");
    let img = image::load_from_memory_with_format(png_bytes, ImageFormat::Png).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(IconData {
        rgba: rgba.as_raw().clone(),
        width,
        height,
    })
}
