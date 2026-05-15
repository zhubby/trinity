use std::sync::Arc;

use eframe::egui::{self, FontDefinitions, FontFamily, TextStyle};
use egui::{FontData, FontId};

use FontFamily::{Monospace, Proportional};

/// Install Chinese font (LXGW WenKai) and set custom font sizes
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

    let font_size_plus = crate::cfg::get_font_size_plus();

    let mut style = (*egui_ctx.global_style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(28.0 + font_size_plus, Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(20.0 + font_size_plus, Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(18.0 + font_size_plus, Monospace),
        ),
        (
            TextStyle::Button,
            FontId::new(20.0 + font_size_plus, Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(18.0 + font_size_plus, Proportional),
        ),
    ]
    .into();

    egui_ctx.set_global_style(style);
}
