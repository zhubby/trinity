//! Trinity GUI - Application entry point
//!
//! Orchestrates all Trinity modules at startup:
//! - Word-selection translation (trinity-translator)
//! - Clipboard management (trinity-clipboard)
//! - Voice dictation (trinity-dictation)
//! - Settings panel (trinity-panel)

#![cfg_attr(not(debug_assertions), deny(warnings))]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    // Initialize shared configuration
    trinity_util::init_config();

    // Initialize each module (currently only translator is fully implemented)
    trinity_clipboard::init();
    trinity_dictation::init();

    // Launch the translator main loop (blocking, platform-specific window management)
    trinity_translator::run();
}
