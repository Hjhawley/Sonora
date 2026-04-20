use iced::widget::{button, container, text_input};
use iced::{Background, Border, Color, Theme};

// Shared palette
pub(crate) const ACCENT: Color = Color::from_rgb8(0x33, 0xAA, 0xBB);
pub(crate) const ACCENT_HOVER: Color = Color::from_rgb8(0x22, 0xFF, 0xCC);

pub(crate) const BG_DARK: Color = Color::from_rgb8(0x19, 0x19, 0x19);
pub(crate) const SURFACE_0: Color = Color::from_rgb8(0x1B, 0x1B, 0x1B);
pub(crate) const SURFACE_1: Color = Color::from_rgb8(0x1E, 0x22, 0x24);
pub(crate) const SURFACE_2: Color = Color::from_rgb8(0x24, 0x33, 0x36);
pub(crate) const SURFACE_3: Color = Color::from_rgb8(0x2A, 0x31, 0x34);

pub(crate) const TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE);
pub(crate) const MUTED_TEXT: Color = Color::from_rgb8(0xC8, 0xC8, 0xC8);
pub(crate) const SECONDARY_TEXT: Color = Color::from_rgb8(0xB0, 0xB0, 0xB0);
pub(crate) const PATH_TEXT: Color = Color::from_rgb8(0x9A, 0x9A, 0x9A);
pub(crate) const DISABLED_TEXT: Color = Color::from_rgb8(0x8E, 0x8E, 0x8E);

pub(crate) const BORDER: Color = Color::from_rgb8(0x3C, 0x3C, 0x3C);
pub(crate) const BORDER_ACTIVE: Color = ACCENT;

// Track/table-specific shared colors
pub(crate) const ROW_BG_EVEN: Color = Color::from_rgb8(0x1B, 0x1B, 0x1B);
pub(crate) const ROW_BG_ODD: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);
pub(crate) const ROW_BG_SELECTED: Color = Color::from_rgb8(0x28, 0x35, 0x39);
pub(crate) const ROW_BG_PLAYING: Color = Color::from_rgb8(0x1F, 0x3D, 0x42);
pub(crate) const ROW_BG_SELECTED_PLAYING: Color = Color::from_rgb8(0x29, 0x50, 0x57);
pub(crate) const ROW_BORDER: Color = Color::from_rgb8(0x2A, 0x2A, 0x2A);

// Shared surface band style for things like the track table header.
pub(crate) fn table_header_band_style() -> container::Style {
    let mut style = container::Style::default();
    style.background = Some(Background::Color(SURFACE_1));
    style.border = Border {
        color: BORDER,
        width: 1.0,
        radius: 0.0.into(),
    };
    style
}

pub(crate) fn track_row_style(
    is_selected: bool,
    is_now_playing: bool,
    zebra_even: bool,
) -> container::Style {
    let bg = match (is_selected, is_now_playing) {
        (true, true) => ROW_BG_SELECTED_PLAYING,
        (false, true) => ROW_BG_PLAYING,
        (true, false) => ROW_BG_SELECTED,
        (false, false) => {
            if zebra_even {
                ROW_BG_EVEN
            } else {
                ROW_BG_ODD
            }
        }
    };

    let border_color = if is_selected || is_now_playing {
        BORDER_ACTIVE
    } else {
        ROW_BORDER
    };

    let mut style = container::Style::default();
    style.background = Some(Background::Color(bg));
    style.border = Border {
        color: border_color,
        width: if is_selected || is_now_playing {
            1.0
        } else {
            0.0
        },
        radius: 0.0.into(),
    };
    style
}

fn flat_button_style(
    theme: &Theme,
    status: button::Status,
    base_bg: Color,
    hover_bg: Color,
    base_text: Color,
    hover_text: Color,
    border_color: Color,
) -> button::Style {
    let mut style = button::secondary(theme, status);

    style.border = Border {
        color: border_color,
        width: 1.0,
        radius: 0.0.into(),
    };

    match status {
        button::Status::Active | button::Status::Pressed => {
            style.background = Some(Background::Color(base_bg));
            style.text_color = base_text;
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(hover_bg));
            style.text_color = hover_text;
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(SURFACE_0));
            style.text_color = DISABLED_TEXT;
        }
    }

    style
}

/// Primary button: still the strongest button, but now uses the flatter table-header language.
pub(crate) fn sonora_button(theme: &Theme, status: button::Status) -> button::Style {
    flat_button_style(
        theme,
        status,
        SURFACE_2,
        SURFACE_3,
        TEXT,
        ACCENT_HOVER,
        BORDER_ACTIVE,
    )
}

/// Muted button: inactive toggles and lower-emphasis controls.
pub(crate) fn sonora_button_muted(theme: &Theme, status: button::Status) -> button::Style {
    flat_button_style(
        theme, status, SURFACE_1, SURFACE_3, MUTED_TEXT, TEXT, BORDER,
    )
}

/// Selected toggle: currently active scope/view.
/// Important: selected toggles remain visually active even when disabled.
pub(crate) fn sonora_button_selected(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::secondary(theme, status);

    style.border = Border {
        color: BORDER_ACTIVE,
        width: 1.0,
        radius: 0.0.into(),
    };

    match status {
        button::Status::Active | button::Status::Pressed | button::Status::Disabled => {
            style.background = Some(Background::Color(SURFACE_2));
            style.text_color = TEXT;
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(SURFACE_3));
            style.text_color = ACCENT_HOVER;
        }
    }

    style
}

/// Dedicated flat header button style for sortable table headers and similar control strips.
pub(crate) fn sonora_header_button(
    active: bool,
    theme: &Theme,
    status: button::Status,
) -> button::Style {
    let mut style = button::secondary(theme, status);

    style.border = Border {
        color: if active { BORDER_ACTIVE } else { BORDER },
        width: 1.0,
        radius: 0.0.into(),
    };

    style.text_color = if active { TEXT } else { MUTED_TEXT };

    match status {
        button::Status::Active | button::Status::Pressed => {
            style.background = Some(Background::Color(if active {
                SURFACE_2
            } else {
                SURFACE_1
            }));
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(if active {
                SURFACE_2
            } else {
                SURFACE_3
            }));
            style.text_color = if active { ACCENT_HOVER } else { TEXT };
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(SURFACE_1));
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
    style.placeholder = DISABLED_TEXT;
    style.value = TEXT;
    style.selection = ACCENT;

    style
}

pub(crate) fn surface_card_style() -> container::Style {
    let mut style = container::Style::default();
    style.background = Some(Background::Color(SURFACE_1));
    style.border = Border {
        color: BORDER,
        width: 1.0,
        radius: 0.0.into(),
    };
    style
}

pub(crate) fn surface_card_style_selected() -> container::Style {
    let mut style = container::Style::default();
    style.background = Some(Background::Color(SURFACE_2));
    style.border = Border {
        color: BORDER_ACTIVE,
        width: 1.0,
        radius: 0.0.into(),
    };
    style
}
