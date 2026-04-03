//! gui/theme.rs
//!
//! Global look and feel for parity between OS
//! Keep layout metrics in 'gui/view/constants.rs'
//! Keep fonts, theme, and app-wide visual defaults here

use iced::theme::Palette;
use iced::{Color, Font, Theme};

/// Bundled font bytes
pub(crate) const APP_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Aileron-Regular.otf");

/// This must match the family name embedded in the font file
pub(crate) const APP_FONT: Font = Font::with_name("Aileron");

/// Force dark mode everywhere with Sonora's own palette
pub(crate) fn app_theme() -> Theme {
    Theme::custom(
        "Sonora Dark",
        Palette {
            background: Color::from_rgb8(0x19, 0x19, 0x19),
            text: Color::from_rgb8(0xEE, 0xEE, 0xEE),
            primary: Color::from_rgb8(0x33, 0xAA, 0xBB),
            success: Color::from_rgb8(0x22, 0xFF, 0xCC),
            warning: Color::from_rgb8(0xFF, 0xCC, 0x66),
            danger: Color::from_rgb8(0xD9, 0x53, 0x4F),
        },
    )
}
