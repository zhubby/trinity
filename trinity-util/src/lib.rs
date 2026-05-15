//! Trinity Util - Shared utilities module
//!
//! Provides shared resources and helpers used across all Trinity modules:
//! - Configuration loading and global settings
//! - Font installation and style customization
//! - Icon loading from embedded PNG resources
//! - Theme management

pub mod cfg;
pub mod font;
pub mod icon;

// Re-export commonly used items for convenience
pub use cfg::{SETTINGS, get_api, get_theme, get_window_size, init_config};
pub use font::install_fonts;
pub use icon::{PNG_BYTES, get_icon_data};
