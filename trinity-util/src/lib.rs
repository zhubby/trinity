//! Trinity Util - Shared utilities module
//!
//! Provides shared resources and helpers used across all Trinity modules:
//! - Configuration loading and global settings
//! - Font installation and style customization
//! - Icon loading from embedded PNG resources
//! - Theme management
//! - Application-wide system hotkey registration

pub mod cfg;
pub mod font;
pub mod hotkey;
pub mod icon;
pub mod persistence;

// Re-export commonly used items for convenience
pub use cfg::{
    SETTINGS, get_api, get_font_size_plus, get_hotkey_config, get_theme, get_window_size,
    init_config, save_basic_config, save_hotkey_config, settings_path,
};
pub use font::install_fonts;
pub use hotkey::{HotkeyAction, HotkeyConfig, HotkeyRegistrationError, HotkeyService};
pub use icon::{LOGO_PNG_BYTES, TRAY_PNG_BYTES, get_icon_data};
