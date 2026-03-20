//! gui/view/tracks.rs
//! Track view (table list).
//!
//! - Row identity is 'TrackId', not 'Vec' index.
//! - Display order is derived from cached Track View query/sort state.
//! - Clicks emit messages by stable id.

use std::time::Instant;

use iced::widget::{
    Column, Row, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Alignment, Length};

use super::super::query::{SortDirection, TrackSortField};
use super::super::state::{LibraryScope, Message, Sonora};
use super::super::util::filename_stem;
use super::constants::{
    HEADER_TEXT, ROW_TEXT, TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::fmt_duration;

const MARKER_W: f32 = 24.0;
const TRACK_NO_W: f32 = 44.0;
const TITLE_W: f32 = 240.0;
const ARTIST_W: f32 = 190.0;
const ALBUM_W: f32 = 240.0;
const ALBUM_ARTIST_W: f32 = 170.0;
const RELEASE_DATE_W: f32 = 110.0;
const GENRE_W: f32 = 140.0;
const LEN_W: f32 = 70.0;

/// Reasonable first-frame fallback before we receive a real viewport height.
const FALLBACK_VIEWPORT_H: f32 = 700.0;

pub(crate) fn build_tracks_center(state: &Sonora) -> Column<'_, Message> {
    let started = Instant::now();

    let title = match state.library_scope {
        LibraryScope::Library => "All Tracks",
        LibraryScope::Hidden => "Hidden Tracks",
        LibraryScope::Missing => "Missing Tracks",
    };

    let visible_ids = &state.track_view_ids;
    let visible_count = visible_ids.len();
    let total_count = state.tracks.iter().filter(|t| t.id.is_some()).count();

    let count_label = if state.track_query.search_text.trim().is_empty() {
        format!("{visible_count} tracks")
    } else {
        format!("{visible_count} of {total_count} tracks")
    };

    let controls = build_track_controls(state);
    let header = build_tracks_header(state);

    let table_started = Instant::now();
    let table = build_tracks_table(state, visible_ids).height(Length::Fill);
    let table_ms = table_started.elapsed().as_secs_f64() * 1000.0;

    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][view::tracks] cached_visible_ids={} total_tracks={} table_ms={:.2} total_ms={:.2}",
        visible_count, total_count, table_ms, total_ms
    );

    column![
        row![text(title).size(18), text(count_label).size(14),]
            .spacing(12)
            .align_y(Alignment::Center),
        controls,
        header,
        table,
    ]
    .spacing(12)
}

fn build_track_controls(state: &Sonora) -> Row<'_, Message> {
    let search = text_input(
        "Search title, artist, album, genre...",
        &state.track_query.search_text,
    )
    .on_input(Message::TrackSearchChanged)
    .padding(8)
    .size(14);

    let clear_button = button(text("Clear").size(14)).on_press(Message::ClearTrackSearch);

    row![search, clear_button]
        .spacing(8)
        .align_y(Alignment::Center)
}

fn build_tracks_header(state: &Sonora) -> Row<'_, Message> {
    row![
        text("").size(HEADER_TEXT).width(Length::Fixed(MARKER_W)),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "#",
            TrackSortField::TrackNo,
            TRACK_NO_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Title",
            TrackSortField::Title,
            TITLE_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Artist",
            TrackSortField::Artist,
            ARTIST_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Album",
            TrackSortField::Album,
            ALBUM_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Album Artist",
            TrackSortField::AlbumArtist,
            ALBUM_ARTIST_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Release Date",
            TrackSortField::ReleaseDate,
            RELEASE_DATE_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Genre",
            TrackSortField::Genre,
            GENRE_W,
        ),
        sort_header_button(
            state.track_query.sort_field,
            state.track_query.sort_direction,
            "Len",
            TrackSortField::Duration,
            LEN_W,
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
}

fn build_tracks_table<'a>(
    state: &'a Sonora,
    visible_ids: &[crate::core::types::TrackId],
) -> iced::widget::Scrollable<'a, Message> {
    let started = Instant::now();

    let row_pitch = TRACK_ROW_H + TRACK_LIST_SPACING;
    let viewport_height = if state.tracks_viewport_height > 1.0 {
        state.tracks_viewport_height
    } else {
        FALLBACK_VIEWPORT_H
    };

    let scroll_y = state.tracks_scroll_offset_y.max(0.0);
    let overscan = state.tracks_overscan_rows;

    let first_visible = (scroll_y / row_pitch).floor() as usize;
    let visible_rows = (viewport_height / row_pitch).ceil() as usize + 1;

    let start_index = first_visible.saturating_sub(overscan);
    let end_index = (first_visible + visible_rows + overscan).min(visible_ids.len());

    let top_spacer_h = (start_index as f32) * row_pitch;
    let bottom_spacer_h = ((visible_ids.len().saturating_sub(end_index)) as f32) * row_pitch;

    let mut col = column![];

    if top_spacer_h > 0.0 {
        col = col.push(
            container(text(""))
                .height(Length::Fixed(top_spacer_h))
                .width(Length::Fill),
        );
    }

    let row_loop_started = Instant::now();

    for &id in &visible_ids[start_index..end_index] {
        let Some(t) = state.track_by_id(id) else {
            continue;
        };

        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);

        let marker = if is_now_playing {
            "▷"
        } else if is_selected {
            "*"
        } else {
            ""
        };

        let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let album = t.album.clone().unwrap_or_else(|| "Unknown".into());
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown".into());
        let release_date = t.release_date.clone().unwrap_or_default();
        let genre = t.genre.clone().unwrap_or_default();
        let len = fmt_duration(t.duration_ms);

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(MARKER_W)),
            text(track_no)
                .size(ROW_TEXT)
                .width(Length::Fixed(TRACK_NO_W)),
            text(title).size(ROW_TEXT).width(Length::Fixed(TITLE_W)),
            text(artist).size(ROW_TEXT).width(Length::Fixed(ARTIST_W)),
            text(album).size(ROW_TEXT).width(Length::Fixed(ALBUM_W)),
            text(album_artist)
                .size(ROW_TEXT)
                .width(Length::Fixed(ALBUM_ARTIST_W)),
            text(release_date)
                .size(ROW_TEXT)
                .width(Length::Fixed(RELEASE_DATE_W)),
            text(genre).size(ROW_TEXT).width(Length::Fixed(GENRE_W)),
            text(len).size(ROW_TEXT).width(Length::Fixed(LEN_W)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::TrackPressed(id));

        col = col.push(row_widget);
    }

    if bottom_spacer_h > 0.0 {
        col = col.push(
            container(text(""))
                .height(Length::Fixed(bottom_spacer_h))
                .width(Length::Fill),
        );
    }

    let row_loop_ms = row_loop_started.elapsed().as_secs_f64() * 1000.0;
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][view::tracks_table] total_rows={} rendered_rows={} start={} end={} offset_y={:.1} viewport_h={:.1} row_loop_ms={:.2} total_ms={:.2}",
        visible_ids.len(),
        end_index.saturating_sub(start_index),
        start_index,
        end_index,
        scroll_y,
        viewport_height,
        row_loop_ms,
        total_ms
    );

    scrollable(col)
        .height(Length::Fill)
        .on_scroll(|viewport| Message::TracksScrolled {
            offset_y: viewport.absolute_offset().y,
            viewport_height: viewport.bounds().height,
        })
}

fn sort_header_button(
    active_field: TrackSortField,
    active_direction: SortDirection,
    label: &'static str,
    field: TrackSortField,
    width: f32,
) -> iced::widget::Button<'static, Message> {
    let suffix = if active_field == field {
        match active_direction {
            SortDirection::Asc => " △",
            SortDirection::Desc => " ▽",
        }
    } else {
        ""
    };

    let text_label = format!("{label}{suffix}");

    button(
        text(text_label)
            .size(HEADER_TEXT)
            .width(Length::Fixed(width)),
    )
    .on_press(Message::SetTrackSortField(field))
}
