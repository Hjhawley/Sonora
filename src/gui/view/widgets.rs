//! Reusable small widgets/helpers used across view modules.

use iced::widget::{column, container, image, row, text, text_input};
use iced::{Alignment, Element, Length};

use super::super::state::Message;
use super::constants::LABEL_W;

pub(crate) fn fmt_duration(ms: Option<u32>) -> String {
    let Some(ms) = ms else { return "-".into() };
    let s = ms / 1000;
    let m = s / 60;
    let s = s % 60;
    format!("{m}:{s:02}")
}

pub(crate) fn cover_placeholder(size: f32) -> iced::widget::Container<'static, Message> {
    container(
        column![text("♪").size(28), text("cover").size(12)]
            .spacing(4)
            .align_x(Alignment::Center),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fill)
    .center_y(Length::Fill)
}

/// If `handle` exists, show it; otherwise show the placeholder.
/// Returns an Element so callers can embed it in `row![]` easily.
pub(crate) fn cover_thumb(
    handle: Option<&iced::widget::image::Handle>,
    size: f32,
) -> Element<'static, Message> {
    match handle {
        Some(h) => container(image(h.clone()))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into(),
        None => cover_placeholder(size).into(),
    }
}

pub(crate) fn field_row<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> iced::widget::Row<'a, Message> {
    row![
        text(label).width(Length::Fixed(LABEL_W)),
        text_input("", value).on_input(on_input).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
}

pub(crate) fn num_pair_row<'a>(
    label: &'a str,
    left: &'a str,
    left_on: impl Fn(String) -> Message + 'a,
    right: &'a str,
    right_on: impl Fn(String) -> Message + 'a,
) -> iced::widget::Row<'a, Message> {
    row![
        text(label).width(Length::Fixed(LABEL_W)),
        text_input("", left)
            .on_input(left_on)
            .width(Length::Fixed(70.0)),
        text("/"),
        text_input("", right)
            .on_input(right_on)
            .width(Length::Fixed(70.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
}

pub(crate) fn playback_bar() -> iced::widget::Container<'static, Message> {
    container(row![text("playback (not yet implemented)").size(28)].align_y(Alignment::Center))
        .padding(16)
}
