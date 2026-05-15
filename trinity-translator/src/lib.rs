//! Trinity Translator - Word-selection translation module
//!
//! Provides word-selection translation functionality including:
//! - Mouse selection detection and text extraction
//! - DeepL translation API calls
//! - Translation result UI display
//! - Selection-copy helpers used by application-level hotkeys
//!
//! # Usage
//!
//! ```no_run
//! trinity_translator::run();
//! ```

pub mod hotkey;
pub mod mouse;
pub mod ui;

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        /// Launch translator main loop (Windows: hotkey mode)
        pub use windows::run;
    } else {
        mod unix;
        /// Launch translator main loop (Unix/macOS: direct window)
        pub use unix::run;
    }
}

pub use mouse::MouseState;
pub use ui::{LINK_COLOR_COMMON, LINK_COLOR_DOING, MyApp, State};

// Re-export shared utilities for convenience
pub use trinity_util::{cfg, font, icon};
