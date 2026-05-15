use std::sync::Arc;

use eframe::egui::{self, FontDefinitions, FontFamily};
use egui::FontData;

/// Install Chinese font (LXGW WenKai) while keeping egui's default text sizes.
pub fn install_fonts(egui_ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "LXGWWenKai-Regular".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/LXGWWenKai-Regular.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "LXGWWenKai-Regular".to_owned());
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, "LXGWWenKai-Regular".to_owned());

    egui_ctx.set_fonts(fonts);
}
