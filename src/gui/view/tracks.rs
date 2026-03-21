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
use super::super::util::{filename_stem, fmt_bitrate_kbps, fmt_channels, fmt_sample_rate_hz};
use super::constants::{
    HEADER_TEXT, ROW_TEXT, TRACK_COL_ALBUM_ARTIST_W, TRACK_COL_ALBUM_W, TRACK_COL_ARTIST_W,
    TRACK_COL_BITRATE_W, TRACK_COL_CHANNELS_W, TRACK_COL_DISC_NO_W, TRACK_COL_GENRE_W,
    TRACK_COL_LEN_W, TRACK_COL_MARKER_W, TRACK_COL_PLAYS_W, TRACK_COL_RATING_W,
    TRACK_COL_RELEASE_DATE_W, TRACK_COL_SAMPLE_RATE_W, TRACK_COL_SPACING, TRACK_COL_TITLE_W,
    TRACK_COL_TRACK_NO_W, TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::{ellipsize_for_width, fmt_duration};

/// Reasonable first-frame fallback before we receive a real viewport height.
const FALLBACK_VIEWPORT_H: f32 = 700.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnKind {
    Marker,
    TrackNo,
    DiscNo,
    Title,
    Artist,
    Album,
    AlbumArtist,
    ReleaseDate,
    Genre,
    Bitrate,
    SampleRate,
    Channels,
    Plays,
    Rating,
    Duration,
}

const TABLE_COLUMNS: [ColumnKind; 15] = [
    ColumnKind::Marker,
    ColumnKind::TrackNo,
    ColumnKind::DiscNo,
    ColumnKind::Title,
    ColumnKind::Artist,
    ColumnKind::Album,
    ColumnKind::AlbumArtist,
    ColumnKind::ReleaseDate,
    ColumnKind::Genre,
    ColumnKind::Bitrate,
    ColumnKind::SampleRate,
    ColumnKind::Channels,
    ColumnKind::Plays,
    ColumnKind::Rating,
    ColumnKind::Duration,
];

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

    let table_started = Instant::now();
    let table = build_tracks_table_region(state, visible_ids).height(Length::Fill);
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
    .size(14)
    .width(Length::Fill);

    let clear_button = button(text("Clear").size(14)).on_press(Message::ClearTrackSearch);

    row![search, clear_button]
        .spacing(8)
        .align_y(Alignment::Center)
}

fn build_tracks_table_region<'a>(
    state: &'a Sonora,
    visible_ids: &'a [crate::core::types::TrackId],
) -> iced::widget::Container<'a, Message> {
    let table_outer_w = table_outer_width();

    let header = build_tracks_header(state);
    let body = build_tracks_body(state, visible_ids);

    let table = column![header, body]
        .spacing(TRACK_LIST_SPACING)
        .width(Length::Fixed(table_outer_w))
        .height(Length::Fill);

    let horizontal = scrollable(table)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::default(),
        ))
        .width(Length::Fill)
        .height(Length::Fill);

    container(horizontal)
        .width(Length::Fill)
        .height(Length::Fill)
}

fn build_tracks_header(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let mut row = row![];
    let col_count = TABLE_COLUMNS.len();

    for (i, col) in TABLE_COLUMNS.iter().enumerate() {
        row = row.push(build_header_cell(
            *col,
            state.track_query.sort_field,
            state.track_query.sort_direction,
        ));

        if i + 1 < col_count {
            row = row.spacing(TRACK_COL_SPACING);
        }
    }

    container(
        row.spacing(TRACK_COL_SPACING)
            .align_y(Alignment::Center)
            .width(Length::Fixed(table_content_width())),
    )
    .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
    .width(Length::Fixed(table_outer_width()))
}

fn build_tracks_body<'a>(
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
    let total_rows = visible_ids.len();

    let raw_first_visible = (scroll_y / row_pitch).floor().max(0.0) as usize;
    let visible_rows = (viewport_height / row_pitch).ceil().max(0.0) as usize + 1;

    let first_visible = raw_first_visible.min(total_rows);
    let start_index = first_visible.saturating_sub(overscan).min(total_rows);
    let end_index = (first_visible + visible_rows + overscan).min(total_rows);

    let top_spacer_h = (start_index as f32) * row_pitch;
    let bottom_spacer_h = ((total_rows.saturating_sub(end_index)) as f32) * row_pitch;

    let mut col = column![];

    if top_spacer_h > 0.0 {
        col = col.push(
            container(text(""))
                .height(Length::Fixed(top_spacer_h))
                .width(Length::Fixed(table_outer_width())),
        );
    }

    let row_loop_started = Instant::now();

    let window = if start_index < end_index {
        &visible_ids[start_index..end_index]
    } else {
        &visible_ids[0..0]
    };

    for &id in window {
        let Some(t) = state.track_by_id(id) else {
            continue;
        };

        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);

        let marker = if is_now_playing {
            "▷"
        } else if is_selected {
            "•"
        } else {
            ""
        };

        let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
        let disc_no = t.disc_no.map(|n| n.to_string()).unwrap_or_default();
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
        let bitrate = fmt_bitrate_kbps(t.bitrate_kbps);
        let sample_rate = fmt_sample_rate_hz(t.sample_rate_hz);
        let channels = fmt_channels(t.channels);
        let plays = t.play_count.map(|n| n.to_string()).unwrap_or_default();
        let rating = t.rating.map(|n| n.to_string()).unwrap_or_default();
        let len = fmt_duration(t.duration_ms);

        let row_cells = row![
            build_text_cell(marker, TRACK_COL_MARKER_W),
            build_text_cell(&track_no, TRACK_COL_TRACK_NO_W),
            build_text_cell(&disc_no, TRACK_COL_DISC_NO_W),
            build_text_cell(&title, TRACK_COL_TITLE_W),
            build_text_cell(&artist, TRACK_COL_ARTIST_W),
            build_text_cell(&album, TRACK_COL_ALBUM_W),
            build_text_cell(&album_artist, TRACK_COL_ALBUM_ARTIST_W),
            build_text_cell(&release_date, TRACK_COL_RELEASE_DATE_W),
            build_text_cell(&genre, TRACK_COL_GENRE_W),
            build_text_cell(&bitrate, TRACK_COL_BITRATE_W),
            build_text_cell(&sample_rate, TRACK_COL_SAMPLE_RATE_W),
            build_text_cell(&channels, TRACK_COL_CHANNELS_W),
            build_text_cell(&plays, TRACK_COL_PLAYS_W),
            build_text_cell(&rating, TRACK_COL_RATING_W),
            build_text_cell(&len, TRACK_COL_LEN_W),
        ]
        .spacing(TRACK_COL_SPACING)
        .align_y(Alignment::Center)
        .width(Length::Fixed(table_content_width()));

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fixed(table_outer_width())),
        )
        .on_press(Message::TrackPressed(id));

        col = col.push(row_widget);
    }

    if bottom_spacer_h > 0.0 {
        col = col.push(
            container(text(""))
                .height(Length::Fixed(bottom_spacer_h))
                .width(Length::Fixed(table_outer_width())),
        );
    }

    let row_loop_ms = row_loop_started.elapsed().as_secs_f64() * 1000.0;
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][view::tracks_table] total_rows={} rendered_rows={} start={} end={} offset_y={:.1} viewport_h={:.1} row_loop_ms={:.2} total_ms={:.2}",
        total_rows,
        end_index.saturating_sub(start_index),
        start_index,
        end_index,
        scroll_y,
        viewport_height,
        row_loop_ms,
        total_ms
    );

    scrollable(
        col.width(Length::Fixed(table_outer_width()))
            .height(Length::Shrink),
    )
    .height(Length::Fill)
    .width(Length::Fixed(table_outer_width()))
    .on_scroll(|viewport| Message::TracksScrolled {
        offset_y: viewport.absolute_offset().y,
        viewport_height: viewport.bounds().height,
    })
}

fn build_header_cell(
    column: ColumnKind,
    active_field: TrackSortField,
    active_direction: SortDirection,
) -> iced::Element<'static, Message> {
    let width = column_width(column);

    match sort_field_for_column(column) {
        Some(field) => sort_header_button(
            active_field,
            active_direction,
            column_label(column),
            field,
            width,
        )
        .into(),
        None => container(
            text(ellipsize_for_width(column_label(column), width))
                .size(HEADER_TEXT)
                .width(Length::Fixed(width)),
        )
        .width(Length::Fixed(width))
        .into(),
    }
}

fn build_text_cell(value: &str, width: f32) -> iced::widget::Container<'static, Message> {
    container(
        text(ellipsize_for_width(value, width))
            .size(ROW_TEXT)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
}

fn column_label(column: ColumnKind) -> &'static str {
    match column {
        ColumnKind::Marker => "",
        ColumnKind::TrackNo => "#",
        ColumnKind::DiscNo => "Disc",
        ColumnKind::Title => "Title",
        ColumnKind::Artist => "Artist",
        ColumnKind::Album => "Album",
        ColumnKind::AlbumArtist => "Album Artist",
        ColumnKind::ReleaseDate => "Release Date",
        ColumnKind::Genre => "Genre",
        ColumnKind::Bitrate => "kbps",
        ColumnKind::SampleRate => "Hz",
        ColumnKind::Channels => "Ch",
        ColumnKind::Plays => "Plays",
        ColumnKind::Rating => "Rate",
        ColumnKind::Duration => "Len",
    }
}

fn sort_field_for_column(column: ColumnKind) -> Option<TrackSortField> {
    match column {
        ColumnKind::Marker => None,
        ColumnKind::TrackNo => Some(TrackSortField::TrackNo),
        ColumnKind::DiscNo => None,
        ColumnKind::Title => Some(TrackSortField::Title),
        ColumnKind::Artist => Some(TrackSortField::Artist),
        ColumnKind::Album => Some(TrackSortField::Album),
        ColumnKind::AlbumArtist => Some(TrackSortField::AlbumArtist),
        ColumnKind::ReleaseDate => Some(TrackSortField::ReleaseDate),
        ColumnKind::Genre => Some(TrackSortField::Genre),
        ColumnKind::Bitrate => None,
        ColumnKind::SampleRate => None,
        ColumnKind::Channels => None,
        ColumnKind::Plays => None,
        ColumnKind::Rating => None,
        ColumnKind::Duration => Some(TrackSortField::Duration),
    }
}

fn column_width(column: ColumnKind) -> f32 {
    match column {
        ColumnKind::Marker => TRACK_COL_MARKER_W,
        ColumnKind::TrackNo => TRACK_COL_TRACK_NO_W,
        ColumnKind::DiscNo => TRACK_COL_DISC_NO_W,
        ColumnKind::Title => TRACK_COL_TITLE_W,
        ColumnKind::Artist => TRACK_COL_ARTIST_W,
        ColumnKind::Album => TRACK_COL_ALBUM_W,
        ColumnKind::AlbumArtist => TRACK_COL_ALBUM_ARTIST_W,
        ColumnKind::ReleaseDate => TRACK_COL_RELEASE_DATE_W,
        ColumnKind::Genre => TRACK_COL_GENRE_W,
        ColumnKind::Bitrate => TRACK_COL_BITRATE_W,
        ColumnKind::SampleRate => TRACK_COL_SAMPLE_RATE_W,
        ColumnKind::Channels => TRACK_COL_CHANNELS_W,
        ColumnKind::Plays => TRACK_COL_PLAYS_W,
        ColumnKind::Rating => TRACK_COL_RATING_W,
        ColumnKind::Duration => TRACK_COL_LEN_W,
    }
}

fn table_content_width() -> f32 {
    let widths_sum: f32 = TABLE_COLUMNS.iter().map(|c| column_width(*c)).sum();
    let gaps = (TABLE_COLUMNS.len().saturating_sub(1) as f32) * TRACK_COL_SPACING;
    widths_sum + gaps
}

fn table_outer_width() -> f32 {
    table_content_width() + (TRACK_ROW_HPAD * 2.0)
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
        text(ellipsize_for_width(&text_label, width))
            .size(HEADER_TEXT)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
    .on_press(Message::SetTrackSortField(field))
}
