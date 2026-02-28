//! gui/view/albums.rs
//! Album view (grouping + album list + detail).
//!
//! - Album grouping stores `TrackId` (stable), not Vec indices.
//! - Cover cache is keyed by `TrackId`.
//! - Track row click emits `Message::SelectTrack(track_id)`.
//!
//! Notes:
//! - We still group from the current `state.tracks` Vec for display.
//! - If a TrackRow has `id: None` (pre-DB), it is skipped to avoid broken messages.

use iced::widget::{Column, column, container, mouse_area, row, scrollable, text};
use iced::{Alignment, Length};
use std::collections::BTreeMap;

use super::super::state::{AlbumKey, Message, Sonora};
use super::super::util::filename_stem;
use super::constants::{
    ALBUM_LIST_H, ALBUM_LIST_SPACING, ALBUM_ROW_COVER, ALBUM_ROW_H, COVER_BIG, ROW_TEXT,
    TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
};
use super::widgets::{cover_thumb, fmt_duration};
use crate::core::types::TrackId;

pub(crate) fn build_albums_center(state: &Sonora) -> Column<'_, Message> {
    let mut groups: BTreeMap<AlbumKey, Vec<TrackId>> = BTreeMap::new();

    for t in state.tracks.iter() {
        let Some(id) = t.id else { continue };

        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        groups
            .entry(AlbumKey {
                album_artist,
                album,
            })
            .or_default()
            .push(id);
    }

    let selected_key: Option<AlbumKey> = state.selected_album.clone();

    // For list display: (key, track_count, representative_track_id)
    // IMPORTANT: do not invent a rep id when an album has no tracks.
    let albums: Vec<(AlbumKey, usize, TrackId)> = groups
        .iter()
        .filter_map(|(k, v)| v.first().copied().map(|rep| (k.clone(), v.len(), rep)))
        .collect();

    let list = build_album_list(state, selected_key.clone(), albums);

    let selected_payload: Option<(AlbumKey, Vec<TrackId>)> = state
        .selected_album
        .as_ref()
        .and_then(|k| groups.get(k).map(|v| (k.clone(), v.clone())));

    let detail = build_album_detail(state, selected_payload);

    column![
        text("Albums").size(18),
        list.height(Length::Fixed(ALBUM_LIST_H)),
        detail.height(Length::Fill),
    ]
    .spacing(12)
}

fn build_album_list(
    state: &Sonora,
    selected: Option<AlbumKey>,
    albums: Vec<(AlbumKey, usize, TrackId)>,
) -> iced::widget::Scrollable<'static, Message> {
    let mut col: Column<'static, Message> = column![].spacing(ALBUM_LIST_SPACING);

    for (key, count, rep_id) in albums {
        let is_selected = selected.as_ref() == Some(&key);

        let title_line = if is_selected {
            format!("● {}", key.album)
        } else {
            key.album.clone()
        };
        let artist_line = key.album_artist.clone();
        let count_line = format!("{count} tracks");

        let cover = cover_thumb(state.cover_cache.get(&rep_id), ALBUM_ROW_COVER);

        let row_cells = row![
            cover,
            column![text(title_line).size(14), text(artist_line).size(12)]
                .spacing(2)
                .width(Length::Fill),
            text(count_line).size(12).width(Length::Fixed(90.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([6, 8])
                .height(Length::Fixed(ALBUM_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectAlbum(key));

        col = col.push(row_widget);
    }

    scrollable(col)
}

fn build_album_detail(
    state: &Sonora,
    selected: Option<(AlbumKey, Vec<TrackId>)>,
) -> iced::widget::Container<'_, Message> {
    let Some((key, track_ids)) = selected else {
        return container(text("Select an album to view tracks.")).padding(12);
    };

    if track_ids.is_empty() {
        return container(text("Album has no tracks (weird).")).padding(12);
    }

    // Resolve ids → indices defensively (avoid panics if the list changed).
    let mut idxs: Vec<usize> = track_ids
        .into_iter()
        .filter_map(|id| state.index_of_id(id))
        .collect();

    if idxs.is_empty() {
        return container(text("Album tracks are out of range (rescan?).")).padding(12);
    }

    // Sort by (disc, track, title) for a sane album ordering.
    idxs.sort_by(|&a, &b| {
        let ta = &state.tracks[a];
        let tb = &state.tracks[b];
        (
            ta.disc_no.unwrap_or(0),
            ta.track_no.unwrap_or(0),
            ta.title.clone().unwrap_or_default(),
        )
            .cmp(&(
                tb.disc_no.unwrap_or(0),
                tb.track_no.unwrap_or(0),
                tb.title.clone().unwrap_or_default(),
            ))
    });

    let first_idx = idxs[0];
    let first = &state.tracks[first_idx];
    let first_id = first.id;

    let year = first
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "-".into());
    let genre = first.genre.clone().unwrap_or_else(|| "-".into());

    // Big cover: use the first track as the representative.
    let big_cover = first_id
        .and_then(|id| state.cover_cache.get(&id))
        .map(|h| cover_thumb(Some(h), COVER_BIG))
        .unwrap_or_else(|| cover_thumb(None, COVER_BIG));

    let header = row![
        big_cover,
        column![
            text(key.album.clone()).size(26),
            text(key.album_artist.clone()).size(18),
            text(format!("{genre} • {year}")).size(14),
            text(format!("{} songs", idxs.len())).size(12),
        ]
        .spacing(6)
        .width(Length::Fill),
    ]
    .spacing(18)
    .align_y(Alignment::Center);

    let mut list = column![].spacing(TRACK_LIST_SPACING);

    for &i in &idxs {
        let t = &state.tracks[i];
        let Some(id) = t.id else { continue };

        let n = t
            .track_no
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".into());
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let dur = fmt_duration(t.duration_ms);

        let is_primary = state.selected_track == Some(id);
        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);

        // Marker rules:
        // - ▶ for now playing (strongest signal)
        // - ● for selected (including the primary selection)
        let marker = if is_now_playing {
            "▶"
        } else if is_selected || is_primary {
            "●"
        } else {
            ""
        };

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(24.0)),
            text(n).size(ROW_TEXT).width(Length::Fixed(32.0)),
            column![text(title).size(ROW_TEXT), text(artist).size(12)]
                .spacing(2)
                .width(Length::Fill),
            text(dur).size(ROW_TEXT).width(Length::Fixed(60.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectTrack(id));

        list = list.push(row_widget);
    }

    let tracks_panel = scrollable(list).height(Length::Fill);
    container(column![header, tracks_panel].spacing(12)).padding(12)
}
