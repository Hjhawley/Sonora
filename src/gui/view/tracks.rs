//! gui/view/tracks.rs
//! Track view (table list).
//!
//! Phase-2 table polish:
//! - reduce Path dominance
//! - improve selection / now-playing affordances
//! - improve sort affordances
//! - slightly improve row scanning
//!
//! - Row identity is 'TrackId', not 'Vec' index.
//! - Display order is derived from cached Track View query/sort state.
//! - Clicks emit messages by stable id.

use std::time::Instant;

use super::style::{sonora_button, sonora_input};
use iced::widget::{
    Column, Row, button, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Length, Theme};

use super::super::query::{SortDirection, TrackSortField};
use super::super::state::{LibraryScope, Message, Sonora};
use super::super::util::{
    ellipsize_path_tail_for_width, filename_stem, fmt_bitrate_kbps, fmt_channels,
    fmt_sample_rate_hz,
};
use super::constants::{
    HEADER_TEXT, ROW_TEXT, TRACK_COL_ALBUM_ARTIST_SORT_W, TRACK_COL_ALBUM_ARTIST_W,
    TRACK_COL_ALBUM_SORT_W, TRACK_COL_ALBUM_W, TRACK_COL_ARTIST_SORT_W, TRACK_COL_ARTIST_W,
    TRACK_COL_ARTWORK_COUNT_W, TRACK_COL_BITRATE_W, TRACK_COL_BPM_W, TRACK_COL_CHANNELS_W,
    TRACK_COL_COMMENT_W, TRACK_COL_COMPILATION_W, TRACK_COL_COMPOSER_W, TRACK_COL_CONDUCTOR_W,
    TRACK_COL_COPYRIGHT_W, TRACK_COL_DISC_NO_W, TRACK_COL_DISC_TOTAL_W, TRACK_COL_ENCODED_BY_W,
    TRACK_COL_ENCODER_SETTINGS_W, TRACK_COL_GENRE_W, TRACK_COL_GROUPING_W, TRACK_COL_ISRC_W,
    TRACK_COL_KEY_W, TRACK_COL_LANGUAGE_W, TRACK_COL_LEN_W, TRACK_COL_LYRICIST_W,
    TRACK_COL_LYRICS_W, TRACK_COL_MARKER_W, TRACK_COL_MOOD_W, TRACK_COL_PATH_W, TRACK_COL_PLAYS_W,
    TRACK_COL_PUBLISHER_W, TRACK_COL_RATING_W, TRACK_COL_RELEASE_DATE_W, TRACK_COL_REMIXER_W,
    TRACK_COL_SAMPLE_RATE_W, TRACK_COL_SPACING, TRACK_COL_SUBTITLE_W, TRACK_COL_TITLE_SORT_W,
    TRACK_COL_TITLE_W, TRACK_COL_TRACK_NO_W, TRACK_COL_TRACK_TOTAL_W, TRACK_COL_YEAR_W,
    TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::{ellipsize_for_width, fmt_duration};

/// Reasonable first-frame fallback before we receive a real viewport height.
const FALLBACK_VIEWPORT_H: f32 = 700.0;

const BUTTON_TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE);
const HEADER_TEXT_MUTED: Color = Color::from_rgb8(0xC8, 0xC8, 0xC8);
const SECONDARY_TEXT: Color = Color::from_rgb8(0xB0, 0xB0, 0xB0);
const PATH_TEXT: Color = Color::from_rgb8(0x9A, 0x9A, 0x9A);

const ACCENT: Color = Color::from_rgb8(0x33, 0xAA, 0xBB);
const ACCENT_HOVER: Color = Color::from_rgb8(0x22, 0xFF, 0xCC);

const HEADER_BG: Color = Color::from_rgb8(0x1E, 0x22, 0x24);
const HEADER_BG_ACTIVE: Color = Color::from_rgb8(0x24, 0x33, 0x36);
const HEADER_BG_HOVER: Color = Color::from_rgb8(0x2A, 0x31, 0x34);
const HEADER_BORDER: Color = Color::from_rgb8(0x3C, 0x3C, 0x3C);

const ROW_BG_EVEN: Color = Color::from_rgb8(0x1B, 0x1B, 0x1B);
const ROW_BG_ODD: Color = Color::from_rgb8(0x1F, 0x1F, 0x1F);
const ROW_BG_SELECTED: Color = Color::from_rgb8(0x28, 0x35, 0x39);
const ROW_BG_PLAYING: Color = Color::from_rgb8(0x1F, 0x3D, 0x42);
const ROW_BG_SELECTED_PLAYING: Color = Color::from_rgb8(0x29, 0x50, 0x57);
const ROW_BORDER: Color = Color::from_rgb8(0x2A, 0x2A, 0x2A);
const ROW_BORDER_ACTIVE: Color = Color::from_rgb8(0x33, 0xAA, 0xBB);

fn button_text<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).color(BUTTON_TEXT)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnKind {
    Marker,
    Path,

    TrackNo,
    TrackTotal,
    DiscNo,
    DiscTotal,

    Title,
    Artist,
    Album,
    AlbumArtist,
    Composer,

    ReleaseDate,
    Year,
    Genre,
    Grouping,
    Comment,
    Lyrics,
    Lyricist,
    Conductor,
    Remixer,
    Publisher,
    Subtitle,
    Bpm,
    Key,
    Mood,
    Language,
    Isrc,
    EncoderSettings,
    EncodedBy,
    Copyright,

    ArtworkCount,
    TitleSort,
    ArtistSort,
    AlbumSort,
    AlbumArtistSort,

    Duration,
    Bitrate,
    SampleRate,
    Channels,
    Rating,
    PlayCount,
    Compilation,
}

const TABLE_COLUMNS: [ColumnKind; 42] = [
    ColumnKind::Marker,
    ColumnKind::Path,
    ColumnKind::TrackNo,
    ColumnKind::TrackTotal,
    ColumnKind::DiscNo,
    ColumnKind::DiscTotal,
    ColumnKind::Title,
    ColumnKind::Artist,
    ColumnKind::Album,
    ColumnKind::AlbumArtist,
    ColumnKind::Composer,
    ColumnKind::ReleaseDate,
    ColumnKind::Year,
    ColumnKind::Genre,
    ColumnKind::Grouping,
    ColumnKind::Comment,
    ColumnKind::Lyrics,
    ColumnKind::Lyricist,
    ColumnKind::Conductor,
    ColumnKind::Remixer,
    ColumnKind::Publisher,
    ColumnKind::Subtitle,
    ColumnKind::Bpm,
    ColumnKind::Key,
    ColumnKind::Mood,
    ColumnKind::Language,
    ColumnKind::Isrc,
    ColumnKind::EncoderSettings,
    ColumnKind::EncodedBy,
    ColumnKind::Copyright,
    ColumnKind::ArtworkCount,
    ColumnKind::TitleSort,
    ColumnKind::ArtistSort,
    ColumnKind::AlbumSort,
    ColumnKind::AlbumArtistSort,
    ColumnKind::Duration,
    ColumnKind::Bitrate,
    ColumnKind::SampleRate,
    ColumnKind::Channels,
    ColumnKind::Rating,
    ColumnKind::PlayCount,
    ColumnKind::Compilation,
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
        row![
            text(title).size(18),
            text(count_label).size(14).color(SECONDARY_TEXT),
        ]
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
    .width(Length::Fill)
    .style(sonora_input);

    let clear_button = button(button_text("Clear").size(14))
        .on_press(Message::ClearTrackSearch)
        .style(sonora_button);

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
    let mut r = row![];

    for (i, col) in TABLE_COLUMNS.iter().enumerate() {
        r = r.push(build_header_cell(
            *col,
            state.track_query.sort_field,
            state.track_query.sort_direction,
        ));

        if i + 1 < TABLE_COLUMNS.len() {
            r = r.spacing(TRACK_COL_SPACING);
        }
    }

    container(
        r.spacing(TRACK_COL_SPACING)
            .align_y(Alignment::Center)
            .width(Length::Fixed(table_content_width())),
    )
    .padding([TRACK_ROW_VPAD + 1.0, TRACK_ROW_HPAD])
    .width(Length::Fixed(table_outer_width()))
    // Header gets its own band so the body scans as a separate region.
    .style(|_theme: &Theme| {
        let mut style = iced::widget::container::Style::default();
        style.background = Some(Background::Color(HEADER_BG));
        style.border = Border {
            color: HEADER_BORDER,
            width: 1.0,
            radius: 0.0.into(),
        };
        style
    })
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

    for (rel_i, &id) in window.iter().enumerate() {
        let Some(t) = state.track_by_id(id) else {
            continue;
        };

        let absolute_index = start_index + rel_i;
        let zebra_even = absolute_index % 2 == 0;

        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);

        let (marker, marker_color) = marker_for_row_state(is_selected, is_now_playing);

        let path = t.path.to_string_lossy().to_string();
        let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
        let track_total = t.track_total.map(|n| n.to_string()).unwrap_or_default();
        let disc_no = t.disc_no.map(|n| n.to_string()).unwrap_or_default();
        let disc_total = t.disc_total.map(|n| n.to_string()).unwrap_or_default();

        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let album = t.album.clone().unwrap_or_else(|| "Unknown".into());
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown".into());
        let composer = t.composer.clone().unwrap_or_default();

        let release_date = t.release_date.clone().unwrap_or_default();
        let year = t.year.map(|n| n.to_string()).unwrap_or_default();
        let genre = t.genre.clone().unwrap_or_default();
        let grouping = t.grouping.clone().unwrap_or_default();
        let comment = t.comment.clone().unwrap_or_default();
        let lyrics = t.lyrics.clone().unwrap_or_default();
        let lyricist = t.lyricist.clone().unwrap_or_default();
        let conductor = t.conductor.clone().unwrap_or_default();
        let remixer = t.remixer.clone().unwrap_or_default();
        let publisher = t.publisher.clone().unwrap_or_default();
        let subtitle = t.subtitle.clone().unwrap_or_default();
        let bpm = t.bpm.map(|n| n.to_string()).unwrap_or_default();
        let key = t.key.clone().unwrap_or_default();
        let mood = t.mood.clone().unwrap_or_default();
        let language = t.language.clone().unwrap_or_default();
        let isrc = t.isrc.clone().unwrap_or_default();
        let encoder_settings = t.encoder_settings.clone().unwrap_or_default();
        let encoded_by = t.encoded_by.clone().unwrap_or_default();
        let copyright = t.copyright.clone().unwrap_or_default();

        let artwork_count = t.artwork_count.to_string();
        let title_sort = t.title_sort.clone().unwrap_or_default();
        let artist_sort = t.artist_sort.clone().unwrap_or_default();
        let album_sort = t.album_sort.clone().unwrap_or_default();
        let album_artist_sort = t.album_artist_sort.clone().unwrap_or_default();

        let len = fmt_duration(t.duration_ms);
        let bitrate = fmt_bitrate_kbps(t.bitrate_kbps);
        let sample_rate = fmt_sample_rate_hz(t.sample_rate_hz);
        let channels = fmt_channels(t.channels);
        let rating = t.rating.map(|n| n.to_string()).unwrap_or_default();
        let play_count = t.play_count.map(|n| n.to_string()).unwrap_or_default();
        let compilation = match t.compilation {
            Some(true) => "true".to_string(),
            Some(false) => "false".to_string(),
            None => String::new(),
        };

        let row_cells = row![
            build_text_cell_with_color(marker, TRACK_COL_MARKER_W, marker_color),
            build_path_cell(&path, TRACK_COL_PATH_W),
            build_text_cell(&track_no, TRACK_COL_TRACK_NO_W),
            build_text_cell(&track_total, TRACK_COL_TRACK_TOTAL_W),
            build_text_cell(&disc_no, TRACK_COL_DISC_NO_W),
            build_text_cell(&disc_total, TRACK_COL_DISC_TOTAL_W),
            build_text_cell(&title, TRACK_COL_TITLE_W),
            build_text_cell(&artist, TRACK_COL_ARTIST_W),
            build_text_cell(&album, TRACK_COL_ALBUM_W),
            build_text_cell(&album_artist, TRACK_COL_ALBUM_ARTIST_W),
            build_text_cell(&composer, TRACK_COL_COMPOSER_W),
            build_text_cell(&release_date, TRACK_COL_RELEASE_DATE_W),
            build_text_cell(&year, TRACK_COL_YEAR_W),
            build_text_cell(&genre, TRACK_COL_GENRE_W),
            build_text_cell(&grouping, TRACK_COL_GROUPING_W),
            build_text_cell(&comment, TRACK_COL_COMMENT_W),
            build_text_cell(&lyrics, TRACK_COL_LYRICS_W),
            build_text_cell(&lyricist, TRACK_COL_LYRICIST_W),
            build_text_cell(&conductor, TRACK_COL_CONDUCTOR_W),
            build_text_cell(&remixer, TRACK_COL_REMIXER_W),
            build_text_cell(&publisher, TRACK_COL_PUBLISHER_W),
            build_text_cell(&subtitle, TRACK_COL_SUBTITLE_W),
            build_text_cell(&bpm, TRACK_COL_BPM_W),
            build_text_cell(&key, TRACK_COL_KEY_W),
            build_text_cell(&mood, TRACK_COL_MOOD_W),
            build_text_cell(&language, TRACK_COL_LANGUAGE_W),
            build_text_cell(&isrc, TRACK_COL_ISRC_W),
            build_text_cell(&encoder_settings, TRACK_COL_ENCODER_SETTINGS_W),
            build_text_cell(&encoded_by, TRACK_COL_ENCODED_BY_W),
            build_text_cell(&copyright, TRACK_COL_COPYRIGHT_W),
            build_text_cell(&artwork_count, TRACK_COL_ARTWORK_COUNT_W),
            build_text_cell(&title_sort, TRACK_COL_TITLE_SORT_W),
            build_text_cell(&artist_sort, TRACK_COL_ARTIST_SORT_W),
            build_text_cell(&album_sort, TRACK_COL_ALBUM_SORT_W),
            build_text_cell(&album_artist_sort, TRACK_COL_ALBUM_ARTIST_SORT_W),
            build_text_cell(&len, TRACK_COL_LEN_W),
            build_text_cell(&bitrate, TRACK_COL_BITRATE_W),
            build_text_cell(&sample_rate, TRACK_COL_SAMPLE_RATE_W),
            build_text_cell(&channels, TRACK_COL_CHANNELS_W),
            build_text_cell(&rating, TRACK_COL_RATING_W),
            build_text_cell(&play_count, TRACK_COL_PLAYS_W),
            build_text_cell(&compilation, TRACK_COL_COMPILATION_W),
        ]
        .spacing(TRACK_COL_SPACING)
        .align_y(Alignment::Center)
        .width(Length::Fixed(table_content_width()));

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fixed(table_outer_width()))
                // Row state now does real visual work instead of relying only on
                // the marker column. This is the biggest table readability win.
                .style(move |_theme: &Theme| {
                    track_row_style(is_selected, is_now_playing, zebra_even)
                }),
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
                .color(HEADER_TEXT_MUTED)
                .width(Length::Fixed(width)),
        )
        .width(Length::Fixed(width))
        .into(),
    }
}

fn build_text_cell(value: &str, width: f32) -> iced::widget::Container<'static, Message> {
    build_text_cell_with_color(value, width, BUTTON_TEXT)
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

fn column_label(column: ColumnKind) -> &'static str {
    match column {
        ColumnKind::Marker => "",
        ColumnKind::Path => "Path",
        ColumnKind::TrackNo => "Track",
        ColumnKind::TrackTotal => "of",
        ColumnKind::DiscNo => "Disc",
        ColumnKind::DiscTotal => "of",
        ColumnKind::Title => "Title",
        ColumnKind::Artist => "Artist",
        ColumnKind::Album => "Album",
        ColumnKind::AlbumArtist => "Album Artist",
        ColumnKind::Composer => "Composer",
        ColumnKind::ReleaseDate => "Release Date",
        ColumnKind::Year => "Year",
        ColumnKind::Genre => "Genre",
        ColumnKind::Grouping => "Grouping",
        ColumnKind::Comment => "Comment",
        ColumnKind::Lyrics => "Lyrics",
        ColumnKind::Lyricist => "Lyricist",
        ColumnKind::Conductor => "Conductor",
        ColumnKind::Remixer => "Remixer",
        ColumnKind::Publisher => "Publisher",
        ColumnKind::Subtitle => "Subtitle",
        ColumnKind::Bpm => "BPM",
        ColumnKind::Key => "Key",
        ColumnKind::Mood => "Mood",
        ColumnKind::Language => "Language",
        ColumnKind::Isrc => "ISRC",
        ColumnKind::EncoderSettings => "Encoder",
        ColumnKind::EncodedBy => "Encoded By",
        ColumnKind::Copyright => "Copyright",
        ColumnKind::ArtworkCount => "Artwork",
        ColumnKind::TitleSort => "Title Sort",
        ColumnKind::ArtistSort => "Artist Sort",
        ColumnKind::AlbumSort => "Album Sort",
        ColumnKind::AlbumArtistSort => "Album Artist Sort",
        ColumnKind::Duration => "Length",
        ColumnKind::Bitrate => "kbps",
        ColumnKind::SampleRate => "Hz",
        ColumnKind::Channels => "Ch",
        ColumnKind::Rating => "Rating",
        ColumnKind::PlayCount => "Plays",
        ColumnKind::Compilation => "Comp",
    }
}

fn sort_field_for_column(column: ColumnKind) -> Option<TrackSortField> {
    match column {
        ColumnKind::Marker => None,
        ColumnKind::Path => Some(TrackSortField::Path),
        ColumnKind::TrackNo => Some(TrackSortField::TrackNo),
        ColumnKind::TrackTotal => Some(TrackSortField::TrackTotal),
        ColumnKind::DiscNo => Some(TrackSortField::DiscNo),
        ColumnKind::DiscTotal => Some(TrackSortField::DiscTotal),
        ColumnKind::Title => Some(TrackSortField::Title),
        ColumnKind::Artist => Some(TrackSortField::Artist),
        ColumnKind::Album => Some(TrackSortField::Album),
        ColumnKind::AlbumArtist => Some(TrackSortField::AlbumArtist),
        ColumnKind::Composer => Some(TrackSortField::Composer),
        ColumnKind::ReleaseDate => Some(TrackSortField::ReleaseDate),
        ColumnKind::Year => Some(TrackSortField::Year),
        ColumnKind::Genre => Some(TrackSortField::Genre),
        ColumnKind::Grouping => Some(TrackSortField::Grouping),
        ColumnKind::Comment => Some(TrackSortField::Comment),
        ColumnKind::Lyrics => Some(TrackSortField::Lyrics),
        ColumnKind::Lyricist => Some(TrackSortField::Lyricist),
        ColumnKind::Conductor => Some(TrackSortField::Conductor),
        ColumnKind::Remixer => Some(TrackSortField::Remixer),
        ColumnKind::Publisher => Some(TrackSortField::Publisher),
        ColumnKind::Subtitle => Some(TrackSortField::Subtitle),
        ColumnKind::Bpm => Some(TrackSortField::Bpm),
        ColumnKind::Key => Some(TrackSortField::Key),
        ColumnKind::Mood => Some(TrackSortField::Mood),
        ColumnKind::Language => Some(TrackSortField::Language),
        ColumnKind::Isrc => Some(TrackSortField::Isrc),
        ColumnKind::EncoderSettings => Some(TrackSortField::EncoderSettings),
        ColumnKind::EncodedBy => Some(TrackSortField::EncodedBy),
        ColumnKind::Copyright => Some(TrackSortField::Copyright),
        ColumnKind::ArtworkCount => Some(TrackSortField::ArtworkCount),
        ColumnKind::TitleSort => Some(TrackSortField::TitleSort),
        ColumnKind::ArtistSort => Some(TrackSortField::ArtistSort),
        ColumnKind::AlbumSort => Some(TrackSortField::AlbumSort),
        ColumnKind::AlbumArtistSort => Some(TrackSortField::AlbumArtistSort),
        ColumnKind::Duration => Some(TrackSortField::Duration),
        ColumnKind::Bitrate => Some(TrackSortField::Bitrate),
        ColumnKind::SampleRate => Some(TrackSortField::SampleRate),
        ColumnKind::Channels => Some(TrackSortField::Channels),
        ColumnKind::Rating => Some(TrackSortField::Rating),
        ColumnKind::PlayCount => Some(TrackSortField::PlayCount),
        ColumnKind::Compilation => Some(TrackSortField::Compilation),
    }
}

fn column_width(column: ColumnKind) -> f32 {
    match column {
        ColumnKind::Marker => TRACK_COL_MARKER_W,
        ColumnKind::Path => TRACK_COL_PATH_W,
        ColumnKind::TrackNo => TRACK_COL_TRACK_NO_W,
        ColumnKind::TrackTotal => TRACK_COL_TRACK_TOTAL_W,
        ColumnKind::DiscNo => TRACK_COL_DISC_NO_W,
        ColumnKind::DiscTotal => TRACK_COL_DISC_TOTAL_W,
        ColumnKind::Title => TRACK_COL_TITLE_W,
        ColumnKind::Artist => TRACK_COL_ARTIST_W,
        ColumnKind::Album => TRACK_COL_ALBUM_W,
        ColumnKind::AlbumArtist => TRACK_COL_ALBUM_ARTIST_W,
        ColumnKind::Composer => TRACK_COL_COMPOSER_W,
        ColumnKind::ReleaseDate => TRACK_COL_RELEASE_DATE_W,
        ColumnKind::Year => TRACK_COL_YEAR_W,
        ColumnKind::Genre => TRACK_COL_GENRE_W,
        ColumnKind::Grouping => TRACK_COL_GROUPING_W,
        ColumnKind::Comment => TRACK_COL_COMMENT_W,
        ColumnKind::Lyrics => TRACK_COL_LYRICS_W,
        ColumnKind::Lyricist => TRACK_COL_LYRICIST_W,
        ColumnKind::Conductor => TRACK_COL_CONDUCTOR_W,
        ColumnKind::Remixer => TRACK_COL_REMIXER_W,
        ColumnKind::Publisher => TRACK_COL_PUBLISHER_W,
        ColumnKind::Subtitle => TRACK_COL_SUBTITLE_W,
        ColumnKind::Bpm => TRACK_COL_BPM_W,
        ColumnKind::Key => TRACK_COL_KEY_W,
        ColumnKind::Mood => TRACK_COL_MOOD_W,
        ColumnKind::Language => TRACK_COL_LANGUAGE_W,
        ColumnKind::Isrc => TRACK_COL_ISRC_W,
        ColumnKind::EncoderSettings => TRACK_COL_ENCODER_SETTINGS_W,
        ColumnKind::EncodedBy => TRACK_COL_ENCODED_BY_W,
        ColumnKind::Copyright => TRACK_COL_COPYRIGHT_W,
        ColumnKind::ArtworkCount => TRACK_COL_ARTWORK_COUNT_W,
        ColumnKind::TitleSort => TRACK_COL_TITLE_SORT_W,
        ColumnKind::ArtistSort => TRACK_COL_ARTIST_SORT_W,
        ColumnKind::AlbumSort => TRACK_COL_ALBUM_SORT_W,
        ColumnKind::AlbumArtistSort => TRACK_COL_ALBUM_ARTIST_SORT_W,
        ColumnKind::Duration => TRACK_COL_LEN_W,
        ColumnKind::Bitrate => TRACK_COL_BITRATE_W,
        ColumnKind::SampleRate => TRACK_COL_SAMPLE_RATE_W,
        ColumnKind::Channels => TRACK_COL_CHANNELS_W,
        ColumnKind::Rating => TRACK_COL_RATING_W,
        ColumnKind::PlayCount => TRACK_COL_PLAYS_W,
        ColumnKind::Compilation => TRACK_COL_COMPILATION_W,
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

fn marker_for_row_state(is_selected: bool, is_now_playing: bool) -> (&'static str, Color) {
    match (is_selected, is_now_playing) {
        // Strongest state: current track and selected.
        (true, true) => ("▷ ", ACCENT_HOVER),
        // Current track should always be obvious.
        (false, true) => ("▷ ", ACCENT),
        (true, false) => ("", HEADER_TEXT_MUTED),
        (false, false) => ("", BUTTON_TEXT),
    }
}

fn track_row_style(
    is_selected: bool,
    is_now_playing: bool,
    zebra_even: bool,
) -> iced::widget::container::Style {
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
        ROW_BORDER_ACTIVE
    } else {
        ROW_BORDER
    };

    let mut style = iced::widget::container::Style::default();
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

fn track_header_button_style(active: bool, theme: &Theme, status: button::Status) -> button::Style {
    // Table headers should feel like sortable column headers, not like primary
    // app action buttons. So we use a flatter local style here instead of the
    // generic cyan action button style.
    let mut style = button::secondary(theme, status);

    style.border = Border {
        color: if active { ACCENT } else { HEADER_BORDER },
        width: 1.0,
        radius: 0.0.into(),
    };

    style.text_color = if active {
        BUTTON_TEXT
    } else {
        HEADER_TEXT_MUTED
    };

    match status {
        button::Status::Active | button::Status::Pressed => {
            style.background = Some(Background::Color(if active {
                HEADER_BG_ACTIVE
            } else {
                HEADER_BG
            }));
        }
        button::Status::Hovered => {
            style.background = Some(Background::Color(if active {
                HEADER_BG_ACTIVE
            } else {
                HEADER_BG_HOVER
            }));
            style.text_color = if active { ACCENT_HOVER } else { BUTTON_TEXT };
        }
        button::Status::Disabled => {
            style.background = Some(Background::Color(HEADER_BG));
            style.text_color = HEADER_TEXT_MUTED;
        }
    }

    style
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
        // Let the button style control text emphasis instead of forcing the same
        // white text for every header.
        text(display_label)
            .size(HEADER_TEXT)
            .width(Length::Fixed(width)),
    )
    .width(Length::Fixed(width))
    .on_press(Message::SetTrackSortField(field))
    .style(move |theme: &Theme, status| track_header_button_style(is_active, theme, status))
}
