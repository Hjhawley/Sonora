//! gui/view/tracks.rs
//!
//! Track view (table list).
//! Track View owns:
//! - column-driven rendering
//! - table header / resize handle logic
//! - virtualization

use std::time::Instant;

use super::shared::{
    action_button_text, heading_row, marker_for_row_state, styled_library_row, title_for_scope,
};
use super::style::{
    MUTED_TEXT, PATH_TEXT, TEXT, sonora_button, sonora_header_button, sonora_input,
    table_header_band_style,
};
use iced::widget::{
    Column, Row, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Color, Length, Theme};

use super::super::columns::{TrackColumn, TrackColumnState};
use super::super::query::{SortDirection, TrackSortField};
use super::super::state::{Message, Sonora};
use super::super::util::{
    ellipsize_path_tail_for_width, filename_stem, fmt_bitrate_kbps, fmt_channels,
    fmt_sample_rate_hz,
};
use super::constants::{
    HEADER_TEXT, ROW_TEXT, TRACK_COL_SPACING, TRACK_LIST_SPACING, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::{ellipsize_for_width, fmt_duration};

const FALLBACK_VIEWPORT_H: f32 = 700.0;
const HEADER_RESIZE_HANDLE_W: f32 = 10.0;

pub(crate) fn build_tracks_center(state: &Sonora) -> Column<'_, Message> {
    let started = Instant::now();

    let title = title_for_scope(
        state.library_scope,
        "All Tracks",
        "Hidden Tracks",
        "Missing Tracks",
    );

    let visible_ids = &state.track_view_ids;
    let visible_count = visible_ids.len();
    let total_count = state
        .tracks
        .iter()
        .filter(|track| track.id.is_some())
        .count();

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

    column![heading_row(title, count_label), controls, table].spacing(12)
}

fn build_track_controls(state: &Sonora) -> Row<'_, Message> {
    let search = text_input(
        "Search title, artist, album, genre...",
        &state.track_query.search_text,
    )
    .on_input(Message::TrackSearchChanged)
    .padding(8)
    .size(14)
    .width(Length::Fill)
    .style(sonora_input);

    let clear_button = button(action_button_text("Clear").size(14))
        .on_press(Message::ClearTrackSearch)
        .style(sonora_button);

    row![search, clear_button].spacing(8)
}

fn build_tracks_table_region<'a>(
    state: &'a Sonora,
    visible_ids: &'a [crate::core::types::TrackId],
) -> iced::widget::Container<'a, Message> {
    let columns = visible_track_columns(state);
    let table_outer_w = table_outer_width(&columns);

    let header = build_tracks_header(state, &columns);
    let body = build_tracks_body(state, visible_ids, &columns);

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

fn build_tracks_header<'a>(
    state: &'a Sonora,
    columns: &[&'a TrackColumnState],
) -> iced::widget::Container<'a, Message> {
    let table_content_w = table_content_width(columns);
    let table_outer_w = table_outer_width(columns);

    let mut row_widget = row![]
        .spacing(TRACK_COL_SPACING)
        .align_y(iced::Alignment::Center)
        .width(Length::Fixed(table_content_w));

    for column in columns {
        row_widget = row_widget.push(build_header_cell(
            column,
            state.track_query.sort_field,
            state.track_query.sort_direction,
        ));
    }

    container(row_widget)
        .padding([TRACK_ROW_VPAD + 1.0, TRACK_ROW_HPAD])
        .width(Length::Fixed(table_outer_w))
        .style(|_theme: &Theme| table_header_band_style())
}

fn build_tracks_body<'a>(
    state: &'a Sonora,
    visible_ids: &[crate::core::types::TrackId],
    columns: &[&TrackColumnState],
) -> iced::widget::Scrollable<'a, Message> {
    let started = Instant::now();

    let table_content_w = table_content_width(columns);
    let table_outer_w = table_outer_width(columns);

    let row_pitch = super::constants::TRACK_ROW_H + TRACK_LIST_SPACING;
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

    let mut rows_column = column![];

    if top_spacer_h > 0.0 {
        rows_column = rows_column.push(
            container(text(""))
                .height(Length::Fixed(top_spacer_h))
                .width(Length::Fixed(table_outer_w)),
        );
    }

    let row_loop_started = Instant::now();

    let window = if start_index < end_index {
        &visible_ids[start_index..end_index]
    } else {
        &visible_ids[0..0]
    };

    for (relative_index, &id) in window.iter().enumerate() {
        let Some(track) = state.track_by_id(id) else {
            continue;
        };

        let absolute_index = start_index + relative_index;
        let zebra_even = absolute_index % 2 == 0;

        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);

        let (marker, marker_color) = marker_for_row_state(is_selected, is_now_playing);

        let mut row_cells = row![]
            .spacing(TRACK_COL_SPACING)
            .align_y(iced::Alignment::Center)
            .width(Length::Fixed(table_content_w));

        for column_state in columns {
            row_cells = row_cells.push(build_row_cell_for_track(
                track,
                column_state,
                marker,
                marker_color,
            ));
        }

        let row_widget = mouse_area(styled_library_row(
            row_cells,
            is_selected,
            is_now_playing,
            zebra_even,
            Length::Fixed(table_outer_w),
        ))
        .on_press(Message::TrackPressed(id));

        rows_column = rows_column.push(row_widget);
    }

    if bottom_spacer_h > 0.0 {
        rows_column = rows_column.push(
            container(text(""))
                .height(Length::Fixed(bottom_spacer_h))
                .width(Length::Fixed(table_outer_w)),
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
        rows_column
            .width(Length::Fixed(table_outer_w))
            .height(Length::Shrink),
    )
    .height(Length::Fill)
    .width(Length::Fixed(table_outer_w))
    .on_scroll(|viewport| Message::TracksScrolled {
        offset_y: viewport.absolute_offset().y,
        viewport_height: viewport.bounds().height,
    })
}

fn visible_track_columns(state: &Sonora) -> Vec<&TrackColumnState> {
    state
        .track_columns
        .iter()
        .filter(|column| column.visible)
        .collect()
}

fn build_header_cell(
    column: &TrackColumnState,
    active_field: TrackSortField,
    active_direction: SortDirection,
) -> iced::Element<'static, Message> {
    let kind = column.kind;
    let total_width = column.width.max(HEADER_RESIZE_HANDLE_W + 12.0);
    let label_width = (total_width - HEADER_RESIZE_HANDLE_W).max(12.0);

    let header_main: iced::Element<'static, Message> = match kind.sort_field() {
        Some(field) => sort_header_button(
            active_field,
            active_direction,
            kind.label(),
            field,
            label_width,
        )
        .into(),
        None => container(
            text(ellipsize_for_width(kind.label(), label_width))
                .size(HEADER_TEXT)
                .color(MUTED_TEXT)
                .width(Length::Fixed(label_width)),
        )
        .width(Length::Fixed(label_width))
        .into(),
    };

    let resize_grip = mouse_area(
        container(text("⋮").size(12).color(MUTED_TEXT))
            .width(Length::Fixed(HEADER_RESIZE_HANDLE_W)),
    )
    .on_press(Message::StartTrackColumnResize(kind));

    container(
        row![header_main, resize_grip]
            .spacing(0)
            .align_y(iced::Alignment::Center)
            .width(Length::Fixed(total_width)),
    )
    .width(Length::Fixed(total_width))
    .into()
}

fn build_row_cell_for_track(
    track: &crate::core::types::TrackRow,
    column: &TrackColumnState,
    marker: &str,
    marker_color: Color,
) -> iced::widget::Container<'static, Message> {
    match column.kind {
        TrackColumn::Marker => build_text_cell_with_color(marker, column.width, marker_color),
        TrackColumn::Path => {
            let path = track.path.to_string_lossy();
            build_path_cell(path.as_ref(), column.width)
        }
        kind => {
            let value = track_value_for_column(track, kind);
            build_text_cell(&value, column.width)
        }
    }
}

fn track_value_for_column(track: &crate::core::types::TrackRow, column: TrackColumn) -> String {
    match column {
        TrackColumn::Marker => String::new(),
        TrackColumn::Path => track.path.to_string_lossy().to_string(),

        TrackColumn::TrackNo => counter_display(&track.track_no_text, track.track_no),
        TrackColumn::TrackTotal => counter_display(&track.track_total_text, track.track_total),
        TrackColumn::DiscNo => counter_display(&track.disc_no_text, track.disc_no),
        TrackColumn::DiscTotal => counter_display(&track.disc_total_text, track.disc_total),

        TrackColumn::Title => track
            .title
            .clone()
            .unwrap_or_else(|| filename_stem(&track.path)),
        TrackColumn::Artist => track.artist.clone().unwrap_or_else(|| "Unknown".into()),
        TrackColumn::Album => track.album.clone().unwrap_or_else(|| "Unknown".into()),
        TrackColumn::AlbumArtist => track
            .album_artist
            .clone()
            .or_else(|| track.artist.clone())
            .unwrap_or_else(|| "Unknown".into()),
        TrackColumn::Composer => track.composer.clone().unwrap_or_default(),

        TrackColumn::ReleaseDate => track.release_date.clone().unwrap_or_default(),
        TrackColumn::Year => track
            .year
            .map(|number| number.to_string())
            .unwrap_or_default(),
        TrackColumn::Genre => track.genre.clone().unwrap_or_default(),
        TrackColumn::Grouping => track.grouping.clone().unwrap_or_default(),
        TrackColumn::ContentGroup => track.content_group.clone().unwrap_or_default(),
        TrackColumn::Comment => track.comment.clone().unwrap_or_default(),
        TrackColumn::Lyrics => track.lyrics.clone().unwrap_or_default(),
        TrackColumn::Lyricist => track.lyricist.clone().unwrap_or_default(),
        TrackColumn::Conductor => track.conductor.clone().unwrap_or_default(),
        TrackColumn::Remixer => track.remixer.clone().unwrap_or_default(),
        TrackColumn::Publisher => track.publisher.clone().unwrap_or_default(),
        TrackColumn::Subtitle => track.subtitle.clone().unwrap_or_default(),
        TrackColumn::Bpm => track
            .bpm
            .map(|number| number.to_string())
            .unwrap_or_default(),
        TrackColumn::Key => track.key.clone().unwrap_or_default(),
        TrackColumn::Mood => track.mood.clone().unwrap_or_default(),
        TrackColumn::Language => track.language.clone().unwrap_or_default(),
        TrackColumn::Isrc => track.isrc.clone().unwrap_or_default(),
        TrackColumn::EncoderSettings => track.encoder_settings.clone().unwrap_or_default(),
        TrackColumn::EncodedBy => track.encoded_by.clone().unwrap_or_default(),
        TrackColumn::Copyright => track.copyright.clone().unwrap_or_default(),

        TrackColumn::ArtworkCount => track.artwork_count.to_string(),
        TrackColumn::TitleSort => track.title_sort.clone().unwrap_or_default(),
        TrackColumn::ArtistSort => track.artist_sort.clone().unwrap_or_default(),
        TrackColumn::AlbumSort => track.album_sort.clone().unwrap_or_default(),
        TrackColumn::AlbumArtistSort => track.album_artist_sort.clone().unwrap_or_default(),

        TrackColumn::Duration => fmt_duration(track.duration_ms),
        TrackColumn::Bitrate => fmt_bitrate_kbps(track.bitrate_kbps),
        TrackColumn::SampleRate => fmt_sample_rate_hz(track.sample_rate_hz),
        TrackColumn::Channels => fmt_channels(track.channels),
        TrackColumn::Rating => track
            .rating
            .map(|number| number.to_string())
            .unwrap_or_default(),
        TrackColumn::PlayCount => track
            .play_count
            .map(|number| number.to_string())
            .unwrap_or_default(),
        TrackColumn::Compilation => match track.compilation {
            Some(true) => "true".to_string(),
            Some(false) => "false".to_string(),
            None => String::new(),
        },
    }
}

fn counter_display(raw: &Option<String>, numeric: Option<u32>) -> String {
    match raw.as_deref() {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => numeric.map(|number| number.to_string()).unwrap_or_default(),
    }
}

fn build_text_cell(value: &str, width: f32) -> iced::widget::Container<'static, Message> {
    build_text_cell_with_color(value, width, TEXT)
}

fn build_text_cell_with_color(
    value: &str,
    width: f32,
    color: Color,
) -> iced::widget::Container<'static, Message> {
    container(
        text(ellipsize_for_width(value, width))
            .size(ROW_TEXT)
            .color(color)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
}

fn build_path_cell(path: &str, width: f32) -> iced::widget::Container<'static, Message> {
    container(
        text(ellipsize_path_tail_for_width(path, width))
            .size(ROW_TEXT)
            .color(PATH_TEXT)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
}

fn table_content_width(columns: &[&TrackColumnState]) -> f32 {
    let widths_sum: f32 = columns.iter().map(|column| column.width).sum();
    let gaps = (columns.len().saturating_sub(1) as f32) * TRACK_COL_SPACING;
    widths_sum + gaps
}

fn table_outer_width(columns: &[&TrackColumnState]) -> f32 {
    table_content_width(columns) + (TRACK_ROW_HPAD * 2.0)
}

fn sort_header_button(
    active_field: TrackSortField,
    active_direction: SortDirection,
    label: &'static str,
    field: TrackSortField,
    width: f32,
) -> iced::widget::Button<'static, Message> {
    let is_active = active_field == field;

    let suffix = if is_active {
        match active_direction {
            SortDirection::Asc => " △",
            SortDirection::Desc => " ▽",
        }
    } else {
        ""
    };

    let text_label = format!("{label}{suffix}");
    let display_label = ellipsize_for_width(&text_label, width);

    button(
        text(display_label)
            .size(HEADER_TEXT)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
    .on_press(Message::SetTrackSortField(field))
    .style(move |theme: &Theme, status| sonora_header_button(is_active, theme, status))
}
