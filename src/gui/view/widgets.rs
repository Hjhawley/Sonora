//! gui/view/widgets.rs
//! Reusable helpers used across view modules.
#![allow(dead_code)]

use iced::widget::{button, column, container, image, row, slider, text, text_input};
use iced::{Alignment, Element, Length};

use super::super::state::{Message, Sonora};
use super::constants::LABEL_W;

pub(crate) fn fmt_duration(ms: Option<u32>) -> String {
    let Some(ms) = ms else { return "-".into() };
    let s = ms / 1000;
    let m = s / 60;
    let s = s % 60;
    format!("{m}:{s:02}")
}

fn fmt_duration_u64(ms: u64) -> String {
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

/// Bottom playback bar.
pub(crate) fn playback_bar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let engine_ready = state.playback.is_some();

    let play_label = if state.is_playing { "Pause" } else { "Play" };

    let prev_btn = if engine_ready {
        button("⏮").on_press(Message::Prev)
    } else {
        button("⏮")
    };

    let play_btn = if engine_ready {
        button(play_label).on_press(Message::TogglePlayPause)
    } else {
        button(play_label)
    };

    let next_btn = if engine_ready {
        button("⏭").on_press(Message::Next)
    } else {
        button("⏭")
    };

    // --- seek slider (ratio 0..=1) ---
    let pos = state.position_ms;
    let dur = state.duration_ms.unwrap_or(0);

    let seek_enabled = engine_ready && dur > 0;

    let seek_val = if dur > 0 {
        (pos as f32 / dur as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let seek = if seek_enabled {
        slider(0.0..=1.0, seek_val, Message::SeekTo).width(Length::Fill)
    } else {
        slider(0.0..=1.0, seek_val, |_| Message::Noop).width(Length::Fill)
    };

    let time_text = if dur > 0 {
        format!("{} / {}", fmt_duration_u64(pos), fmt_duration_u64(dur))
    } else {
        format!("{} / -:--", fmt_duration_u64(pos))
    };

    // --- volume slider ---
    let vol = state.volume.clamp(0.0, 1.0);

    let vol_slider = if engine_ready {
        slider(0.0..=1.0, vol, Message::SetVolume).width(Length::Fixed(140.0))
    } else {
        slider(0.0..=1.0, vol, |_| Message::Noop).width(Length::Fixed(140.0))
    };

    // --- now playing label ---
    let now_playing = match state.now_playing.and_then(|i| state.tracks.get(i)) {
        Some(t) => t
            .title
            .clone()
            .or_else(|| t.path.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Unknown".into()),
        None => "Nothing playing".into(),
    };

    let bar = row![
        row![prev_btn, play_btn, next_btn]
            .spacing(8)
            .align_y(Alignment::Center),
        column![
            text(now_playing).size(14),
            row![seek, text(time_text).size(12)]
                .spacing(10)
                .align_y(Alignment::Center),
        ]
        .spacing(6)
        .width(Length::Fill),
        row![text("Vol").size(12), vol_slider]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(16)
    .align_y(Alignment::Center);

    container(bar).padding(12)
}
