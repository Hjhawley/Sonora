//! Reusable small widgets/helpers used across view modules.

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
///
/// Emits only Messages (no rodio, no decoding).
///
/// Requires these Sonora fields:
/// - is_playing: bool
/// - position_ms: u64
/// - duration_ms: Option<u64>
/// - volume: f32
/// - now_playing: Option<usize>   (track index)
pub(crate) fn playback_bar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let play_label = if state.is_playing { "Pause" } else { "Play" };

    let prev_btn = button("⏮").on_press(Message::Prev);
    let play_btn = button(play_label).on_press(Message::TogglePlayPause);
    let next_btn = button("⏭").on_press(Message::Next);

    // --- seek slider ---
    let pos = state.position_ms;
    let dur = state.duration_ms.unwrap_or(0);
    let seek_enabled = dur > 0;

    // slider needs a sane range; if we don't know duration yet, freeze it at 0..=1
    let (seek_min, seek_max, seek_val) = if seek_enabled {
        (0.0f32, dur as f32, pos.min(dur) as f32)
    } else {
        (0.0f32, 1.0f32, 0.0f32)
    };

    let seek = slider(seek_min..=seek_max, seek_val, Message::SeekTo).width(Length::Fill);

    let time_text = if seek_enabled {
        format!("{} / {}", fmt_duration_u64(pos), fmt_duration_u64(dur))
    } else {
        // show position even if duration unknown
        format!("{} / -:--", fmt_duration_u64(pos))
    };

    // --- volume slider ---
    // clamp for sanity; slider requires value within bounds
    let vol = state.volume.clamp(0.0, 1.0);
    let vol_slider = slider(0.0..=1.0, vol, Message::SetVolume).width(Length::Fixed(140.0));

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
        // left: transport
        row![prev_btn, play_btn, next_btn]
            .spacing(8)
            .align_y(Alignment::Center),
        // middle: now playing + seek
        column![
            text(now_playing).size(14),
            row![seek, text(time_text).size(12)]
                .spacing(10)
                .align_y(Alignment::Center),
        ]
        .spacing(6)
        .width(Length::Fill),
        // right: volume
        row![text("Vol").size(12), vol_slider]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(16)
    .align_y(Alignment::Center);

    container(bar).padding(12)
}
