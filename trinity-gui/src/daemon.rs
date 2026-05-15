//! DaemonApp — invisible background eframe app that manages the system tray
//!
//! This app has a hidden main viewport and serves as the daemon backbone:
//! - Creates and polls the system tray for menu events
//! - Shows the Settings Panel viewport when requested
//! - Closes the app when "Exit" is selected
//! - Spawns background translator threads (mouse listener, translation engine)
//! - Shows the Translator popup viewport when a word selection triggers translation

use eframe::App;
use egui::{Context, ViewportBuilder, ViewportCommand, ViewportId};
use log::warn;
use std::sync::{Arc, Mutex, mpsc};

use crate::tray::TrayEvent;
use trinity_panel::PanelApp;
use trinity_util::{
    cfg::{get_theme, get_window_size},
    font::install_fonts,
};

/// The background daemon application. Its main viewport is invisible;
/// all user-facing windows are spawned as secondary viewports.
pub struct DaemonApp {
    /// Channel to receive tray events (ShowPanel / Exit)
    tray_rx: mpsc::Receiver<TrayEvent>,
    /// Whether the Settings Panel viewport is currently visible
    panel_visible: bool,
    /// The PanelApp instance that draws the settings panel viewport
    panel_app: Option<PanelApp>,
    /// Whether the tray has been created (deferred to first ui() call on macOS)
    tray_created: bool,
    /// Shared translation state for background translator threads
    translator_state: Arc<Mutex<TranslatorState>>,
    /// Channel to signal a new translation popup should appear
    translator_popup_tx: mpsc::SyncSender<TranslationPopupEvent>,
    /// Channel to receive signals that a translation popup should appear
    translator_popup_rx: mpsc::Receiver<TranslationPopupEvent>,
}

/// Translation popup event carrying the text to display
struct TranslationPopupEvent {
    text: String,
}

/// Background translation state shared between threads
struct TranslatorState {
    /// Current translation text (shown in popup)
    text: String,
}

impl DaemonApp {
    /// Create a new DaemonApp.
    ///
    /// The tray is created lazily on the first `ui()` call because macOS
    /// requires NSApp to be initialized first (eframe sets this up).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);

        // Apply theme
        match get_theme().as_str() {
            "light" => cc.egui_ctx.set_visuals(egui::Visuals::light()),
            _ => cc.egui_ctx.set_visuals(egui::Visuals::dark()),
        }

        // Channels for tray communication — dummy receiver until tray is created
        let (_, tray_rx) = mpsc::channel();

        // Channels for translation popup
        let (popup_tx, popup_rx) = mpsc::sync_channel(1);

        // Shared state for background translator
        let translator_state = Arc::new(Mutex::new(TranslatorState {
            text: String::new(),
        }));

        Self {
            tray_rx,
            panel_visible: false,
            panel_app: None,
            tray_created: false,
            translator_state,
            translator_popup_tx: popup_tx,
            translator_popup_rx: popup_rx,
        }
    }
}

impl App for DaemonApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // ── Deferred tray creation (first frame) ───────────────────────
        if !self.tray_created {
            self.tray_created = true;
            let (tray_tx, _tray_rx) = mpsc::channel();
            self.tray_rx = crate::tray::create_tray(tray_tx);

            // ── Spawn background translator threads ────────────────────
            spawn_translator_threads(
                ctx.clone(),
                self.translator_state.clone(),
                self.translator_popup_tx.clone(),
            );
        }

        // ── Process tray events ─────────────────────────────────────────
        while let Ok(event) = self.tray_rx.try_recv() {
            match event {
                TrayEvent::ShowPanel => {
                    self.panel_visible = true;
                    if self.panel_app.is_none() {
                        // We create a PanelApp context lazily. Since PanelApp::new()
                        // needs a CreationContext, we'll create a simple panel
                        // that uses the daemon's context directly.
                        self.panel_app = Some(PanelApp::new_from_context(&ctx));
                    }
                }
                TrayEvent::Exit => {
                    // Close all viewports and exit
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    return;
                }
            }
        }

        // ── Show Settings Panel viewport ────────────────────────────────
        if self.panel_visible
            && let Some(panel_app) = &self.panel_app
        {
            let (width, height) = get_window_size();
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("panel"),
                ViewportBuilder::default()
                    .with_title("Trinity Settings")
                    .with_inner_size([width, height])
                    .with_resizable(true),
                |ui, _class| {
                    // Draw the panel content using PanelApp's ui logic
                    panel_app.show_inside(ui);
                },
            );
        }

        // ── Process translation popup events ────────────────────────────
        while let Ok(popup) = self.translator_popup_rx.try_recv() {
            let (width, height) = get_window_size();
            ctx.show_viewport_immediate(
                ViewportId::from_hash_of("translator_popup"),
                ViewportBuilder::default()
                    .with_title("Translator")
                    .with_always_on_top()
                    .with_decorations(false)
                    .with_inner_size([width, height]),
                |ui, _class| {
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        ui.label(&popup.text);
                    });
                },
            );
        }

        // ── Keep daemon alive by requesting repaint ─────────────────────
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Spawn the background translator threads:
/// 1. Mouse listener (rdev::listen) — detects word selection
/// 2. Translation poller — waits for selection, runs DeepL translation,
///    then signals popup via channel
fn spawn_translator_threads(
    ctx: Context,
    state: Arc<Mutex<TranslatorState>>,
    popup_tx: mpsc::SyncSender<TranslationPopupEvent>,
) {
    use std::{
        thread::{self, sleep},
        time::Duration,
    };

    // ── Mouse listener thread ───────────────────────────────────
    {
        let mouse_state = Arc::new(Mutex::new(trinity_translator::MouseState::new()));
        let mouse_state_clone = mouse_state.clone();

        thread::spawn(move || {
            if let Err(err) = rdev::listen(move |event| {
                let mut ms = mouse_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                match event.event_type {
                    rdev::EventType::ButtonPress(rdev::Button::Left) => ms.down(),
                    rdev::EventType::ButtonRelease(rdev::Button::Left) => ms.release(),
                    rdev::EventType::MouseMove { .. } => ms.moving(),
                    _ => {}
                }
            }) {
                warn!("rdev listen error: {:?}", err);
            }
        });

        // ── Translation poller thread ────────────────────────────
        thread::spawn(move || {
            let mut clipboard_last = String::new();
            loop {
                let is_select = {
                    let mut ms = mouse_state.lock().unwrap_or_else(|e| e.into_inner());
                    ms.is_select()
                };

                if is_select
                    && !ctx.input(|i| i.pointer.has_pointer())
                    && let Some(text_new) = trinity_translator::hotkey::ctrl_c()
                    && text_new != clipboard_last
                {
                    clipboard_last = text_new.clone();

                    // Store text in shared state
                    {
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.text = text_new.clone();
                    }

                    // Translate
                    let (source_lang, target_lang) = {
                        // Default languages; in future read from shared settings
                        (deepl::Lang::Auto, deepl::Lang::ZH)
                    };
                    let result = deepl::translate(
                        &trinity_util::cfg::get_api(),
                        text_new,
                        target_lang,
                        source_lang,
                    )
                    .unwrap_or_else(|_| "翻译接口失效，请更换".to_string());

                    // Update shared state and signal popup
                    {
                        let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                        s.text = result.clone();
                    }
                    popup_tx.send(TranslationPopupEvent { text: result }).ok();
                    ctx.request_repaint();
                }

                sleep(Duration::from_millis(100));
            }
        });
    }
}
