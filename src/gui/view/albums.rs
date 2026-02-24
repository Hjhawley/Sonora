//! gui/view/albums.rs
//! Album view (grouping + album list + detail).

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

pub(crate) fn build_albums_center(state: &Sonora) -> Column<'_, Message> {
    let mut groups: BTreeMap<AlbumKey, Vec<usize>> = BTreeMap::new();

    for (i, t) in state.tracks.iter().enumerate() {
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
            .push(i);
    }

    let selected_key: Option<AlbumKey> = state.selected_album.clone();

    // For list display: (key, track_count, representative_track_index)
    // IMPORTANT: do not invent a rep index (like 0) when an album has no tracks.
    let albums: Vec<(AlbumKey, usize, usize)> = groups
        .iter()
        .filter_map(|(k, v)| v.first().map(|&rep| (k.clone(), v.len(), rep)))
        .collect();

    let list = build_album_list(state, selected_key.clone(), albums);

    let selected_payload: Option<(AlbumKey, Vec<usize>)> = state
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
    albums: Vec<(AlbumKey, usize, usize)>,
) -> iced::widget::Scrollable<'static, Message> {
    let mut col: Column<'static, Message> = column![].spacing(ALBUM_LIST_SPACING);

    for (key, count, rep_idx) in albums {
        let is_selected = selected.as_ref() == Some(&key);

        let title_line = if is_selected {
            format!("● {}", key.album)
        } else {
            key.album.clone()
        };
        let artist_line = key.album_artist.clone();
        let count_line = format!("{count} tracks");

        let cover = cover_thumb(state.cover_cache.get(&rep_idx), ALBUM_ROW_COVER);

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
    selected: Option<(AlbumKey, Vec<usize>)>,
) -> iced::widget::Container<'_, Message> {
    let Some((key, track_idxs)) = selected else {
        return container(text("Select an album to view tracks.")).padding(12);
    };

    if track_idxs.is_empty() {
        return container(text("Album has no tracks (weird).")).padding(12);
    }

    // Defensive: filter out any out-of-range indices (shouldn't happen, but avoids panics).
    let mut idxs: Vec<usize> = track_idxs
        .into_iter()
        .filter(|&i| i < state.tracks.len())
        .collect();

    if idxs.is_empty() {
        return container(text("Album tracks are out of range (rescan?).")).padding(12);
    }

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

    let year = first
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "-".into());
    let genre = first.genre.clone().unwrap_or_else(|| "-".into());

    let big_cover = cover_thumb(state.cover_cache.get(&first_idx), COVER_BIG);

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
        let n = t
            .track_no
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".into());
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let dur = fmt_duration(t.duration_ms);

        let is_primary = state.selected_track == Some(i);
        let is_selected = state.selected_tracks.contains(&i);

        let marker = if is_primary {
            "▶"
        } else if is_selected {
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
        .on_press(Message::SelectTrack(i));

        list = list.push(row_widget);
    }

    let tracks_panel = scrollable(list).height(Length::Fill);
    container(column![header, tracks_panel].spacing(12)).padding(12)
}
