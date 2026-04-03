//! gui/theme.rs
//!
//! Global look and feel for parity between OS
//! Keep layout metrics in 'gui/view/constants.rs'
//! Keep fonts, theme, and app-wide visual defaults here

use iced::{Font, Theme};

/// Bundled font bytes
pub(crate) const APP_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Aileron-Regular.otf");

/// This must match the family name embedded in the font file
pub(crate) const APP_FONT: Font = Font::with_name("Aileron");

/// Global default text size for the app
pub(crate) const DEFAULT_TEXT_SIZE: f32 = 14.0;

/// Force dark mode everywhere
pub(crate) fn app_theme() -> Theme {
    Theme::Dark
}
