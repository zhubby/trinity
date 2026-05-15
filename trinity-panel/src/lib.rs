//! Trinity Panel - Settings and control panel GUI
//!
//! Provides a settings panel for the Trinity application where users can:
//! - Configure translation API settings
//! - Configure hotkeys
//! - Switch themes (dark/light)
//! - Manage clipboard preferences
//! - Manage dictation preferences
//!
//! This module is currently a stub. Full implementation is planned.

use trinity_util::cfg::get_theme;

/// Initialize the panel module
pub fn init() {
    // TODO: implement panel initialization
}

/// Settings panel application
pub struct PanelApp;

impl PanelApp {
    /// Create a new panel app with the given creation context
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply theme from config
        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }
        Self
    }
}

impl eframe::App for PanelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Trinity Settings");
            ui.separator();
            ui.label("Settings panel — coming soon.");
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("⚙").size(60.0));
                ui.add_space(10.0);
                ui.label("Configuration panel will be available here.");
            });
        });
    }
}
