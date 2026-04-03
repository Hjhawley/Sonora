//! gui/view/widgets.rs
//! Reusable helpers used across view modules.
#![allow(dead_code)]

use std::sync::OnceLock;

use iced::widget::{button, column, container, image, row, slider, text, text_input};
use iced::{Alignment, Color, Element, Length};

use super::super::state::{Message, PlayOrder, PlaybackContext, RepeatMode, Sonora};
use super::constants::{LABEL_W, PLAYBACK_COVER};
use super::style::{sonora_button, sonora_input};

const BUTTON_TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE);
const SECONDARY_TEXT: Color = Color::from_rgb8(0xB8, 0xB8, 0xB8);

// Shared centered lane for both transport buttons and progress row.
const PLAYBACK_CENTER_LANE_W: f32 = 760.0;

// Keep the left and right playback clusters the same width so the transport
// cluster stays visually centered in the full bar.
const PLAYBACK_SIDE_CLUSTER_W: f32 = 280.0;

/// Real placeholder artwork instead of the old text-based placeholder.
/// This makes the now-playing block feel more like part of a music app and less
/// like a temporary utility stub.
const PLACEHOLDER_COVER_BYTES: &[u8] = include_bytes!("../../../assets/placeholder.jpg");

pub(crate) fn fmt_duration(ms: Option<u32>) -> String {
    let Some(ms) = ms else {
        return "-".into();
    };
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

/// Truncation helper for table/grid cells
pub(crate) fn ellipsize_for_width(value: &str, width: f32) -> String {
    let text = value.trim();
    if text.is_empty() {
        return String::new();
    }

    let approx_chars = ((width - 10.0) / 7.2).floor().max(1.0) as usize;

    let char_count = text.chars().count();
    if char_count <= approx_chars {
        return text.to_string();
    }

    if approx_chars <= 1 {
        return "…".to_string();
    }

    let keep = approx_chars.saturating_sub(1);
    let mut out = text.chars().take(keep).collect::<String>();
    out.push('…');
    out
}

fn button_text<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).color(BUTTON_TEXT)
}

fn placeholder_cover_handle() -> iced::widget::image::Handle {
    // Cache the placeholder once.
    // Rebuilding Handle::from_bytes(...) every frame causes pointless work and,
    // in practice, visible flashing when no artwork is present.
    static HANDLE: OnceLock<iced::widget::image::Handle> = OnceLock::new();

    HANDLE
        .get_or_init(|| iced::widget::image::Handle::from_bytes(PLACEHOLDER_COVER_BYTES))
        .clone()
}

pub(crate) fn cover_placeholder(size: f32) -> iced::widget::Container<'static, Message> {
    container(
        image(placeholder_cover_handle())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size)),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
}

/// If 'handle' exists, show it; otherwise show the placeholder.
pub(crate) fn cover_thumb(
    handle: Option<&iced::widget::image::Handle>,
    size: f32,
) -> Element<'static, Message> {
    match handle {
        Some(h) => container(
            image(h.clone())
                .width(Length::Fixed(size))
                .height(Length::Fixed(size)),
        )
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
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
        text_input("", value)
            .on_input(on_input)
            .width(Length::Fill)
            .style(sonora_input),
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
            .width(Length::Fixed(70.0))
            .style(sonora_input),
        text("/"),
        text_input("", right)
            .on_input(right_on)
            .width(Length::Fixed(70.0))
            .style(sonora_input),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
}

/// Top playback bar
pub(crate) fn playback_bar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let engine_ready = state.playback.is_some();
    let play_label = if state.is_playing { "Pause" } else { "Play" };

    let prev_btn = if engine_ready {
        button(button_text("Prev"))
            .on_press(Message::Prev)
            .style(sonora_button)
    } else {
        button(button_text("Prev")).style(sonora_button)
    };

    let play_btn = if engine_ready {
        button(button_text(play_label))
            .on_press(Message::TogglePlayPause)
            .style(sonora_button)
    } else {
        button(button_text(play_label)).style(sonora_button)
    };

    let next_btn = if engine_ready {
        button(button_text("Next"))
            .on_press(Message::Next)
            .style(sonora_button)
    } else {
        button(button_text("Next")).style(sonora_button)
    };

    let shuffle_label = match state.play_order {
        PlayOrder::Normal => "Shuffle",
        PlayOrder::Shuffle => "Shuffle ✓",
    };

    let repeat_label = match state.repeat_mode {
        RepeatMode::Off => "Repeat: Off",
        RepeatMode::All => "Repeat: All",
        RepeatMode::One => "Repeat: One",
    };

    let queue_label = match &state.playback_context {
        PlaybackContext::Library => "Queue: Library".to_string(),
        PlaybackContext::Album(key) => format!("Queue: Album — {}", key.album),
    };

    let shuffle_btn = if engine_ready {
        button(button_text(shuffle_label))
            .on_press(Message::ToggleShuffle)
            .style(sonora_button)
    } else {
        button(button_text(shuffle_label)).style(sonora_button)
    };

    let repeat_btn = if engine_ready {
        button(button_text(repeat_label))
            .on_press(Message::CycleRepeatMode)
            .style(sonora_button)
    } else {
        button(button_text(repeat_label)).style(sonora_button)
    };

    let pos = state.position_ms;
    let dur = state.duration_ms.unwrap_or(0);

    let seek_enabled = engine_ready && dur > 0;

    let live_ratio = if dur > 0 {
        (pos as f32 / dur as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let shown_ratio = state.seek_preview_ratio.unwrap_or(live_ratio);

    let seek = if seek_enabled {
        slider(0.0..=1.0, shown_ratio, Message::SeekTo)
            .step(0.001)
            .on_release(Message::SeekCommit)
            .width(Length::Fill)
    } else {
        slider(0.0..=1.0, shown_ratio, |_| Message::Noop)
            .step(0.001)
            .width(Length::Fill)
    };

    let vol = state.volume.clamp(0.0, 1.0);

    let vol_slider = if engine_ready {
        slider(0.0..=1.0, vol, Message::SetVolume)
            .step(0.01)
            .width(Length::Fixed(140.0))
    } else {
        slider(0.0..=1.0, vol, |_| Message::Noop)
            .step(0.01)
            .width(Length::Fixed(140.0))
    };

    let now_playing_row = state.now_playing.and_then(|id| state.track_by_id(id));

    let now_playing_title = match now_playing_row {
        Some(t) => t
            .title
            .clone()
            .or_else(|| t.path.file_stem().map(|s| s.to_string_lossy().to_string()))
            .unwrap_or_else(|| "Unknown".into()),
        None => "Nothing playing".into(),
    };

    let now_playing_artist = match now_playing_row {
        Some(t) => t.artist.clone().unwrap_or_else(|| "Unknown Artist".into()),
        None => String::new(),
    };

    let now_playing_cover = state.now_playing.and_then(|id| state.cover_cache.get(&id));

    let now_playing_meta = if now_playing_row.is_some() {
        column![
            text("Now Playing").size(11).color(SECONDARY_TEXT),
            text(now_playing_title).size(15),
            text(now_playing_artist).size(12).color(SECONDARY_TEXT),
            text(queue_label).size(11).color(SECONDARY_TEXT),
        ]
        .spacing(2)
    } else {
        column![
            text("Now Playing").size(11).color(SECONDARY_TEXT),
            text(now_playing_title).size(15),
            text(queue_label).size(11).color(SECONDARY_TEXT),
        ]
        .spacing(2)
    };

    let left_block = row![
        cover_thumb(now_playing_cover, PLAYBACK_COVER),
        now_playing_meta.width(Length::Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .width(Length::Fixed(PLAYBACK_SIDE_CLUSTER_W));

    let transport_row = row![prev_btn, play_btn, next_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    let progress_row = row![
        text(fmt_duration_u64(pos)).size(12).color(SECONDARY_TEXT),
        seek,
        text(if dur > 0 {
            fmt_duration_u64(dur)
        } else {
            "-:--".to_string()
        })
        .size(12)
        .color(SECONDARY_TEXT),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    // Put both transport and progress inside the exact same centered lane.
    // This is the actual fix: both rows now share the same geometry.
    let center_lane = column![
        container(transport_row)
            .width(Length::Fill)
            .center_x(Length::Fill),
        progress_row,
    ]
    .spacing(10)
    .width(Length::Fixed(PLAYBACK_CENTER_LANE_W));

    let center_block = container(center_lane)
        .width(Length::Fill)
        .center_x(Length::Fill);

    let right_block = column![
        row![shuffle_btn, repeat_btn]
            .spacing(8)
            .align_y(Alignment::Center),
        row![text("Volume").size(12).color(SECONDARY_TEXT), vol_slider]
            .spacing(8)
            .align_y(Alignment::Center),
    ]
    .spacing(10)
    .width(Length::Fixed(PLAYBACK_SIDE_CLUSTER_W));

    let bar = row![left_block, center_block, right_block]
        .spacing(18)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    container(bar).padding(12).width(Length::Fill)
}
