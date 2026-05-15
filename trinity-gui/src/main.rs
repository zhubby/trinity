//! Trinity GUI - Application entry point
//!
//! The application starts as a **background daemon** — no window is shown.
//! A system tray icon appears in the OS status bar with a menu:
//! - "Show Settings Panel" → opens the settings window
//! - "Exit" → quits the application
//!
//! Background threads run continuously:
//! - Mouse/selection listener → detects word selections
//! - Translation engine → runs DeepL translation on selections
//! - Translation popup → appears briefly when a translation completes

#![cfg_attr(not(debug_assertions), deny(warnings))]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod daemon;
mod tray;

fn main() {
    // Initialize shared configuration
    trinity_util::init_config();

    // Initialize stub modules (no GUI yet)
    trinity_clipboard::init();
    trinity_dictation::init();

    // Launch the daemon — invisible main viewport + system tray
    launch_daemon();
}

fn launch_daemon() {
    // The daemon's main viewport is invisible — it serves only as the
    // event loop backbone. All user-facing windows are secondary viewports
    // triggered by tray menu or background threads.
    let viewport = egui::ViewportBuilder::default().with_visible(false); // hidden — no window shown on startup

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Trinity",
        native_options,
        Box::new(|cc| Ok(Box::new(daemon::DaemonApp::new(cc)))),
    )
    .ok();
}
