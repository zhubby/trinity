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
use trinity_util::{
    cfg::get_hotkey_config,
    hotkey::{HotkeyEvent, HotkeyService, install_global_hotkey_event_forwarder},
};

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
    let mut logger = env_logger::Builder::new();
    logger.filter_level(level);
    logger
        .filter_module("egui_wgpu", LevelFilter::Warn)
        .filter_module("wgpu", LevelFilter::Warn)
        .filter_module("wgpu_core", LevelFilter::Warn)
        .filter_module("wgpu_hal", LevelFilter::Warn)
        .filter_module("naga", LevelFilter::Warn);
    logger.init();

    // Initialize shared configuration
    trinity_util::init_config();

    // Initialize stub modules (no GUI yet)
    trinity_clipboard::init();
    trinity_dictation::init();

    // Launch the daemon — control panel root viewport + system tray
    let hotkey_config = get_hotkey_config();
    let (hotkey_event_tx, hotkey_event_rx) = std::sync::mpsc::channel::<HotkeyEvent>();
    let hotkey_service = match HotkeyService::new(&hotkey_config) {
        Ok(service) => {
            log::info!("hotkeys initialized before eframe startup");
            if let Err(err) =
                install_global_hotkey_event_forwarder(&hotkey_config, hotkey_event_tx.clone())
            {
                log::warn!("failed to install global hotkey event forwarder: {err}");
            }
            Some(service)
        }
        Err(err) => {
            log::warn!("failed to initialize hotkeys before eframe startup: {err}");
            None
        }
    };
    launch_daemon(hotkey_service, hotkey_event_tx, hotkey_event_rx);
}

fn launch_daemon(
    hotkey_service: Option<HotkeyService>,
    hotkey_event_tx: std::sync::mpsc::Sender<HotkeyEvent>,
    hotkey_event_rx: std::sync::mpsc::Receiver<HotkeyEvent>,
) {
    let (width, height) = trinity_util::cfg::get_window_size();
    // The root viewport is the borderless control panel. Close hides it
    // instead of exiting the daemon; tray/menu Exit performs the full quit.
    let viewport = egui::ViewportBuilder::default()
        .with_title("Trinity Control Panel")
        .with_inner_size([width, height])
        .with_position([100.0, 100.0])
        .with_decorations(false)
        .with_resizable(true);

    let native_options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "Trinity",
        native_options,
        Box::new(|cc| {
            Ok(Box::new(daemon::DaemonApp::new(
                cc,
                hotkey_service,
                hotkey_event_tx,
                hotkey_event_rx,
            )))
        }),
    )
    .ok();
}
