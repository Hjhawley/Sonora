//! gui/view/albums.rs
//! Album view:
//! - album grid when no album is selected
//! - album detail screen when an album is selected
//!
//! Album View owns:
//! - album grid layout
//! - album detail layout
//!
//! Shared library chrome / row language lives in 'shared.rs'.

use iced::widget::{button, column, container, mouse_area, responsive, row, scrollable, text};
use iced::{Alignment, Length, Size};

use super::super::state::{AlbumKey, Message, Sonora};
use super::super::util::filename_stem;
use super::constants::{
    ALBUM_DETAIL_COVER, ALBUM_DETAIL_TRACK_W_LEN, ALBUM_DETAIL_TRACK_W_NO, ALBUM_GRID_MIN_COLS,
    ALBUM_GRID_SPACING_X, ALBUM_GRID_SPACING_Y, ALBUM_TILE_COVER, ALBUM_TILE_W, CARD_PAD,
    HEADER_BLOCK_PAD, HEADER_META_SPACING, PANEL_GROUP_SPACING, ROW_TEXT, TRACK_LIST_SPACING,
};
use super::shared::{
    action_button_text, heading_row, marker_for_row_state, styled_library_row, title_for_scope,
    toolbar_row,
};
use super::style::{
    SECONDARY_TEXT, TEXT, surface_card_style, surface_card_style_selected, table_header_band_style,
};
use super::widgets::{cover_thumb, fmt_duration};
use crate::core::types::TrackId;

#[derive(Clone)]
struct AlbumTile {
    key: AlbumKey,
    count: usize,
    cover: Option<iced::widget::image::Handle>,
}

pub(crate) fn build_albums_center(state: &Sonora) -> iced::widget::Column<'_, Message> {
    match &state.selected_album {
        Some(key) => build_album_detail_screen(state, key.clone()),
        None => build_album_grid_screen(state),
    }
}

fn build_album_grid_screen(state: &Sonora) -> iced::widget::Column<'_, Message> {
    let heading = title_for_scope(
        state.library_scope,
        "All Albums",
        "Hidden Albums",
        "Missing Albums",
    );

    let albums: Vec<AlbumTile> = state
        .album_groups
        .iter()
        .filter_map(|(key, ids)| {
            let rep_id: TrackId = state.representative_cover_track_id(key)?;
            Some(AlbumTile {
                key: key.clone(),
                count: ids.len(),
                cover: state.cover_cache.get(&rep_id).cloned(),
            })
        })
        .collect();

    let count_label = format!("{} albums", albums.len());
    let selected_album = state.selected_album.clone();

    let grid = responsive(move |size: Size| {
        build_album_grid_for_width(&albums, selected_album.as_ref(), size.width).into()
    });

    column![heading_row(heading, count_label), grid.height(Length::Fill)].spacing(12)
}

fn build_album_grid_for_width(
    albums: &[AlbumTile],
    selected_album: Option<&AlbumKey>,
    available_width: f32,
) -> iced::widget::Scrollable<'static, Message> {
    let footprint = ALBUM_TILE_W + ALBUM_GRID_SPACING_X;
    let computed_cols = ((available_width + ALBUM_GRID_SPACING_X) / footprint).floor() as usize;
    let cols = computed_cols.max(ALBUM_GRID_MIN_COLS).max(1);

    let mut rows_col = column![].spacing(ALBUM_GRID_SPACING_Y);

    for chunk in albums.chunks(cols) {
        let mut r = row![].spacing(ALBUM_GRID_SPACING_X);

        for album in chunk {
            let is_selected = selected_album == Some(&album.key);
            let cover = cover_thumb(album.cover.as_ref(), ALBUM_TILE_COVER);

            let tile_body = column![
                cover,
                text(album.key.album.clone())
                    .size(15)
                    .color(TEXT)
                    .width(Length::Fixed(ALBUM_TILE_W)),
                text(album.key.album_artist.clone())
                    .size(12)
                    .color(SECONDARY_TEXT)
                    .width(Length::Fixed(ALBUM_TILE_W)),
                text(format!("{} tracks", album.count))
                    .size(11)
                    .color(SECONDARY_TEXT)
                    .width(Length::Fixed(ALBUM_TILE_W)),
            ]
            .spacing(6)
            .width(Length::Fixed(ALBUM_TILE_W));

            let tile_widget = mouse_area(
                container(tile_body)
                    .width(Length::Fixed(ALBUM_TILE_W))
                    .padding(CARD_PAD)
                    .style(move |_| {
                        if is_selected {
                            surface_card_style_selected()
                        } else {
                            surface_card_style()
                        }
                    }),
            )
            .on_press(Message::AlbumTilePressed(album.key.clone()));

            r = r.push(tile_widget);
        }

        rows_col = rows_col.push(r);
    }

    scrollable(rows_col).height(Length::Fill)
}

fn build_album_detail_screen(state: &Sonora, key: AlbumKey) -> iced::widget::Column<'_, Message> {
    let Some(track_ids) = state.album_groups.get(&key).cloned() else {
        return column![text("Album not found.").size(18)];
    };

    if track_ids.is_empty() {
        return column![text("Album has no tracks.").size(18)];
    }

    let mut idxs: Vec<usize> = track_ids
        .into_iter()
        .filter_map(|id| state.index_of_id(id))
        .collect();

    if idxs.is_empty() {
        return column![text("Album tracks are out of range (rescan?).").size(18)];
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

    let first = &state.tracks[idxs[0]];
    let release_date = first.release_date.clone().unwrap_or_else(|| "-".into());

    let total_tracks = idxs.len();
    let total_minutes: u32 = idxs
        .iter()
        .filter_map(|&i| state.tracks[i].duration_ms)
        .sum::<u32>()
        / 1000
        / 60;

    let count_label = format!("{total_tracks} tracks • {total_minutes} min");

    let back_btn = button(action_button_text("◁ Back to albums"))
        .on_press(Message::SetViewMode(super::super::state::ViewMode::Albums))
        .style(super::style::sonora_button);

    let play_album_btn = button(action_button_text("Play Album"))
        .on_press(Message::PlayAlbum(key.clone()))
        .style(super::style::sonora_button);

    let rep_id = state.representative_cover_track_id(&key);
    let big_cover = rep_id
        .and_then(|id| state.cover_cache.get(&id))
        .map(|h| cover_thumb(Some(h), ALBUM_DETAIL_COVER))
        .unwrap_or_else(|| cover_thumb(None, ALBUM_DETAIL_COVER));

    let toolbar = toolbar_row([back_btn.into(), play_album_btn.into()]);

    let meta_block = column![
        text(key.album.clone()).size(30).color(TEXT),
        text(key.album_artist.clone())
            .size(19)
            .color(SECONDARY_TEXT),
        text(release_date).size(13).color(SECONDARY_TEXT),
        text(count_label.clone()).size(13).color(SECONDARY_TEXT),
    ]
    .spacing(HEADER_META_SPACING)
    .width(Length::Fill);

    let header_content = row![big_cover, meta_block]
        .spacing(20)
        .align_y(Alignment::Center);

    let header = mouse_area(
        container(header_content)
            .width(Length::Fill)
            .padding(HEADER_BLOCK_PAD)
            .style(|_| table_header_band_style()),
    )
    .on_press(Message::AlbumHeaderPressed(key.clone()));

    let mut list = column![].spacing(TRACK_LIST_SPACING);

    for (row_i, &track_idx) in idxs.iter().enumerate() {
        let track = &state.tracks[track_idx];
        let Some(id) = track.id else {
            continue;
        };

        let track_no = track
            .track_no
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".into());
        let title = track
            .title
            .clone()
            .unwrap_or_else(|| filename_stem(&track.path));
        let artist = track.artist.clone().unwrap_or_else(|| "Unknown".into());
        let duration = fmt_duration(track.duration_ms);

        let is_selected = state.selected_tracks.contains(&id);
        let is_now_playing = state.now_playing == Some(id);
        let zebra_even = row_i % 2 == 0;

        let (marker, marker_color) = marker_for_row_state(is_selected, is_now_playing);

        let row_cells = row![
            text(marker)
                .size(ROW_TEXT)
                .color(marker_color)
                .width(Length::Fixed(24.0)),
            text(track_no)
                .size(ROW_TEXT)
                .color(TEXT)
                .width(Length::Fixed(ALBUM_DETAIL_TRACK_W_NO)),
            column![
                text(title).size(ROW_TEXT).color(TEXT),
                text(artist).size(12).color(SECONDARY_TEXT),
            ]
            .spacing(2)
            .width(Length::Fill),
            text(duration)
                .size(ROW_TEXT)
                .color(SECONDARY_TEXT)
                .width(Length::Fixed(ALBUM_DETAIL_TRACK_W_LEN)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(styled_library_row(
            row_cells,
            is_selected,
            is_now_playing,
            zebra_even,
            Length::Fill,
        ))
        .on_press(Message::AlbumTrackPressed(key.clone(), id));

        list = list.push(row_widget);
    }

    column![
        heading_row(key.album.clone(), count_label),
        toolbar,
        header,
        scrollable(list).height(Length::Fill),
    ]
    .spacing(PANEL_GROUP_SPACING)
}
