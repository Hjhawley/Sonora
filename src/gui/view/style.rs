//! gui/view/style.rs
use iced::border;
use iced::widget::{button, text_input};
use iced::{Background, Color, Theme};

const PRIMARY: Color = Color::from_rgb8(0x33, 0xAA, 0xBB); // #3ab
const PRIMARY_HOVER: Color = Color::from_rgb8(0x22, 0xFF, 0xCC); // #2fc
const BG_DARK: Color = Color::from_rgb8(0x19, 0x19, 0x19); // #191919
const ALT_BG: Color = Color::from_rgb8(0x22, 0x22, 0x22); // #222
const TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE); // #eee
const MUTED_TEXT: Color = Color::from_rgb8(0xAA, 0xAA, 0xAA);
const BORDER: Color = Color::from_rgb8(0x44, 0x44, 0x44); // #444

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
            style.text_color = MUTED_TEXT;
        }
    }

    style
}

pub(crate) fn sonora_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);

    style.background = Background::Color(BG_DARK);
    style.border.color = BORDER;
    style.icon = TEXT;
    style.placeholder = MUTED_TEXT;
    style.value = TEXT;
    style.selection = PRIMARY;

    style
}
