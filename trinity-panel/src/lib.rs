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

use egui::{Context, RichText};
use trinity_util::cfg::{SETTINGS, get_theme};

/// Initialize the panel module
pub fn init() {
    // TODO: implement panel initialization
}

/// Settings panel application
pub struct PanelApp {
    /// Current API URL (editable)
    api_url: String,
    /// Current theme selection
    theme: String,
}

impl PanelApp {
    /// Create a new PanelApp from an egui Context (for daemon viewport usage).
    ///
    /// Unlike `new(cc: &CreationContext)` which is for standalone eframe apps,
    /// this constructor uses the daemon's existing context.
    pub fn new_from_context(ctx: &Context) -> Self {
        // Read current settings
        let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        let api_url = settings
            .get_string("api")
            .unwrap_or_else(|_| "https://deepl.zu1k.com/translate".to_string());
        let theme = settings
            .get_string("window.theme")
            .unwrap_or_else(|_| "dark".to_string());

        // Apply theme
        match theme.as_str() {
            "light" => ctx.set_visuals(egui::Visuals::light()),
            _ => ctx.set_visuals(egui::Visuals::dark()),
        }

        Self { api_url, theme }
    }

    /// Create a new panel app with the given creation context (for standalone use)
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply theme from config
        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }

        let settings = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
        let api_url = settings
            .get_string("api")
            .unwrap_or_else(|_| "https://deepl.zu1k.com/translate".to_string());
        let theme = settings
            .get_string("window.theme")
            .unwrap_or_else(|_| "dark".to_string());

        Self { api_url, theme }
    }

    /// Draw the panel contents inside an existing egui Ui.
    ///
    /// This is used by DaemonApp's `show_viewport_immediate` callback
    /// since the panel is drawn as an immediate-mode viewport, not
    /// as a standalone eframe::App.
    pub fn show_inside(&self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Trinity Settings");
            ui.separator();

            // ── API Configuration ─────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Translation API").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("API URL:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.api_url.clone()).desired_width(300.0),
                    );
                });
                ui.horizontal(|ui| {
                    if ui.button("Save API URL").clicked() {
                        // TODO: persist config change
                        log::info!("API URL save requested: {}", self.api_url);
                    }
                    ui.label(
                        RichText::new("(changes not persisted yet)")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });

            ui.add_space(8.0);

            // ── Theme ──────────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Theme").strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Color scheme:");
                    egui::ComboBox::from_id_salt("theme_combo")
                        .selected_text(&self.theme)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.theme.clone(),
                                "dark".to_string(),
                                "Dark",
                            );
                            ui.selectable_value(
                                &mut self.theme.clone(),
                                "light".to_string(),
                                "Light",
                            );
                        });
                });
            });

            ui.add_space(8.0);

            // ── Hotkey Configuration ────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("Hotkeys").strong());
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(RichText::new("⌨").size(40.0));
                    ui.add_space(10.0);
                    ui.label("Hotkey configuration will be available here.");
                });
            });

            ui.add_space(8.0);

            // ── About ──────────────────────────────────────────────────
            ui.group(|ui| {
                ui.label(RichText::new("About").strong());
                ui.add_space(4.0);
                ui.label("Trinity v0.5.0");
                ui.label("Desktop AI trifecta assistant");
                ui.hyperlink_to("GitHub", "https://github.com/zu1k/translator");
            });
        });
    }
}

impl eframe::App for PanelApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show_inside(ui);
    }
}
