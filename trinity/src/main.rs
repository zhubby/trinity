//! Trinity — Application entry point
//!
//! The application starts with the control panel visible.
//! A system tray icon appears in the OS status bar with a menu:
//! - "Show Control Panel" → opens the control panel window
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

use clap::Parser;
use log::LevelFilter;

/// Trinity — Desktop AI trifecta assistant
#[derive(Parser)]
#[command(name = "trinity", version, about)]
struct Cli {
    /// Log level (off, error, warn, info, debug, trace)
    #[arg(long, default_value = "debug", value_name = "LEVEL")]
    log_level: String,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logger with the configured level
    let level = match cli.log_level.to_lowercase() {
        s if s == "off" => LevelFilter::Off,
        s if s == "error" => LevelFilter::Error,
        s if s == "warn" => LevelFilter::Warn,
        s if s == "info" => LevelFilter::Info,
        s if s == "debug" => LevelFilter::Debug,
        s if s == "trace" => LevelFilter::Trace,
        other => {
            eprintln!("Unknown log level '{other}', using debug");
            LevelFilter::Debug
        }
    };
    env_logger::Builder::new().filter_level(level).init();

    // Initialize shared configuration
    trinity_util::init_config();

    // Initialize stub modules (no GUI yet)
    trinity_clipboard::init();
    trinity_dictation::init();

    // Launch the daemon — control panel root viewport + system tray
    launch_daemon();
}

fn launch_daemon() {
    let (width, height) = trinity_util::cfg::get_window_size();
    // The root viewport is the borderless control panel. Close hides it
    // instead of exiting the daemon; tray/menu Exit performs the full quit.
    let viewport = egui::ViewportBuilder::default()
        .with_title("Trinity Control Panel")
        .with_inner_size([width, height])
        .with_position([100.0, 100.0])
        .with_decorations(false)
        .with_resizable(true)
        .with_transparent(true);

    let native_options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Trinity",
        native_options,
        Box::new(|cc| Ok(Box::new(daemon::DaemonApp::new(cc)))),
    )
    .ok();
}
