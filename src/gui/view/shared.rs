//! gui/view/shared.rs
//! Shared library-view helpers used by Track View, Album View, sidebar, and inspector.

use iced::widget::{Column, Row, column, container, row, text};
use iced::{Alignment, Color, Length};

use super::super::state::{LibraryScope, Message};
use super::constants::{
    BUTTON_ROW_SPACING, PANEL_GROUP_SPACING, PANEL_SECTION_SPACING, PANEL_SUBGROUP_SPACING,
    TOOLBAR_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::style::{ACCENT, ACCENT_HOVER, MUTED_TEXT, SECONDARY_TEXT, TEXT, track_row_style};

pub(crate) fn title_for_scope(
    scope: LibraryScope,
    library_title: &'static str,
    hidden_title: &'static str,
    missing_title: &'static str,
) -> &'static str {
    match scope {
        LibraryScope::Library => library_title,
        LibraryScope::Hidden => hidden_title,
        LibraryScope::Missing => missing_title,
    }
}

pub(crate) fn heading_row(
    title: impl Into<String>,
    count_label: impl Into<String>,
) -> Row<'static, Message> {
    row![
        text(title.into()).size(18),
        text(count_label.into()).size(14).color(SECONDARY_TEXT),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
}

pub(crate) fn section_title<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).size(14)
}

pub(crate) fn helper_text<'a>(label: impl Into<String>) -> iced::widget::Text<'static> {
    text(label.into()).size(12).color(SECONDARY_TEXT)
}

pub(crate) fn action_button_text<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).color(TEXT)
}

pub(crate) fn toolbar_row<'a>(
    children: impl IntoIterator<Item = iced::Element<'a, Message>>,
) -> Row<'a, Message> {
    children
        .into_iter()
        .fold(row![], |row, child| row.push(child))
        .spacing(TOOLBAR_SPACING)
        .align_y(Alignment::Center)
}

pub(crate) fn button_row<'a>(
    children: impl IntoIterator<Item = iced::Element<'a, Message>>,
) -> Row<'a, Message> {
    children
        .into_iter()
        .fold(row![], |row, child| row.push(child))
        .spacing(BUTTON_ROW_SPACING)
        .align_y(Alignment::Center)
}

pub(crate) fn section_block<'a>(
    title: &'a str,
    content: impl Into<iced::Element<'a, Message>>,
) -> Column<'a, Message> {
    column![section_title(title), content.into()].spacing(PANEL_SUBGROUP_SPACING)
}

pub(crate) fn panel_stack<'a>(
    children: impl IntoIterator<Item = iced::Element<'a, Message>>,
) -> Column<'a, Message> {
    children
        .into_iter()
        .fold(column![], |col, child| col.push(child))
        .spacing(PANEL_SECTION_SPACING)
}

pub(crate) fn content_group<'a>(
    children: impl IntoIterator<Item = iced::Element<'a, Message>>,
) -> Column<'a, Message> {
    children
        .into_iter()
        .fold(column![], |col, child| col.push(child))
        .spacing(PANEL_GROUP_SPACING)
}

pub(crate) fn marker_for_row_state(
    is_selected: bool,
    is_now_playing: bool,
) -> (&'static str, Color) {
    match (is_selected, is_now_playing) {
        (true, true) => ("▷ ", ACCENT_HOVER),
        (false, true) => ("▷ ", ACCENT),
        (true, false) => ("", MUTED_TEXT),
        (false, false) => ("", TEXT),
    }
}

pub(crate) fn styled_library_row<'a>(
    content: impl Into<iced::Element<'a, Message>>,
    is_selected: bool,
    is_now_playing: bool,
    zebra_even: bool,
    width: Length,
) -> iced::widget::Container<'a, Message> {
    container(content)
        .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
        .height(Length::Fixed(TRACK_ROW_H))
        .width(width)
        .style(move |_theme| track_row_style(is_selected, is_now_playing, zebra_even))
}
