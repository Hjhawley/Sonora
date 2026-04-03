use iced::border;
use iced::widget::{button, text_input};
use iced::{Background, Color, Theme};

const PRIMARY: Color = Color::from_rgb8(0x33, 0xAA, 0xBB); // #3ab
const PRIMARY_HOVER: Color = Color::from_rgb8(0x22, 0xFF, 0xCC); // #2fc

const BG_DARK: Color = Color::from_rgb8(0x19, 0x19, 0x19); // #191919
const ALT_BG: Color = Color::from_rgb8(0x22, 0x22, 0x22); // #222
const MUTED_BG: Color = Color::from_rgb8(0x2A, 0x2A, 0x2A);
const MUTED_BG_HOVER: Color = Color::from_rgb8(0x36, 0x36, 0x36);

const TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE); // #eee
const MUTED_TEXT: Color = Color::from_rgb8(0xC8, 0xC8, 0xC8);
const DISABLED_TEXT: Color = Color::from_rgb8(0x8E, 0x8E, 0x8E);

const BORDER: Color = Color::from_rgb8(0x44, 0x44, 0x44); // #444

/// Accent button: use for active toggles and primary actions.
pub(crate) fn sonora_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::primary(theme, status);

    style.text_color = TEXT;
    style.border = border::rounded(0.0);

    match status {
        button::Status::Active | button::Status::Pressed => {
            style.background = Some(Background::Color(PRIMARY));
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(PRIMARY_HOVER));
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(ALT_BG));
            style.text_color = DISABLED_TEXT;
        }
    }

    style
}

/// Muted button: use for inactive toggles.
pub(crate) fn sonora_button_muted(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::secondary(theme, status);

    style.text_color = MUTED_TEXT;
    style.border = border::rounded(0.0);

    match status {
        button::Status::Active | button::Status::Pressed => {
            style.background = Some(Background::Color(MUTED_BG));
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(MUTED_BG_HOVER));
            style.text_color = TEXT;
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(ALT_BG));
            style.text_color = DISABLED_TEXT;
        }
    }

    style
}

pub(crate) fn sonora_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);

    style.background = Background::Color(BG_DARK);
    style.border.color = BORDER;
    style.icon = TEXT;
    style.placeholder = DISABLED_TEXT;
    style.value = TEXT;
    style.selection = PRIMARY;

    style
}
