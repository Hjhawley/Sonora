//! gui/view/albums.rs
//! Album view:
//! - album grid when no album is selected
//! - album detail screen when an album is selected

use iced::widget::{button, column, container, mouse_area, responsive, row, scrollable, text};
use iced::{Alignment, Length, Size};

use super::super::state::{AlbumKey, LibraryScope, Message, Sonora};
use super::super::util::filename_stem;
use super::constants::{
    ALBUM_DETAIL_COVER, ALBUM_DETAIL_TRACK_W_LEN, ALBUM_DETAIL_TRACK_W_NO, ALBUM_GRID_MIN_COLS,
    ALBUM_GRID_SPACING_X, ALBUM_GRID_SPACING_Y, ALBUM_TILE_COVER, ALBUM_TILE_W, ROW_TEXT,
    TRACK_LIST_SPACING, TRACK_ROW_H, TRACK_ROW_HPAD, TRACK_ROW_VPAD,
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
    let heading = match state.library_scope {
        LibraryScope::Library => "Albums",
        LibraryScope::Hidden => "Hidden Albums",
    };

    let albums: Vec<AlbumTile> = state
        .album_groups
        .iter()
        .filter_map(|(k, v)| {
            let rep_id: TrackId = v.first().copied()?;
            Some(AlbumTile {
                key: k.clone(),
                count: v.len(),
                cover: state.cover_cache.get(&rep_id).cloned(),
            })
        })
        .collect();

    let grid = responsive(move |size: Size| build_album_grid_for_width(&albums, size.width).into());

    column![text(heading).size(18), grid.height(Length::Fill)].spacing(18)
}

fn build_album_grid_for_width(
    albums: &[AlbumTile],
    available_width: f32,
) -> iced::widget::Scrollable<'static, Message> {
    // Grid layout
    let footprint = ALBUM_TILE_W + ALBUM_GRID_SPACING_X;
    let computed_cols = ((available_width + ALBUM_GRID_SPACING_X) / footprint).floor() as usize;
    let cols = computed_cols.max(ALBUM_GRID_MIN_COLS).max(1);

    let mut rows_col = column![].spacing(ALBUM_GRID_SPACING_Y);

    for chunk in albums.chunks(cols) {
        let mut r = row![].spacing(ALBUM_GRID_SPACING_X);

        for album in chunk {
            let cover = cover_thumb(album.cover.as_ref(), ALBUM_TILE_COVER);

            let tile = column![
                cover,
                text(album.key.album.clone())
                    .size(15)
                    .width(Length::Fixed(ALBUM_TILE_W)),
                text(album.key.album_artist.clone())
                    .size(12)
                    .width(Length::Fixed(ALBUM_TILE_W)),
                text(format!("{} tracks", album.count))
                    .size(11)
                    .width(Length::Fixed(ALBUM_TILE_W)),
            ]
            .spacing(4)
            .width(Length::Fixed(ALBUM_TILE_W));

            let tile_widget = mouse_area(
                container(tile)
                    .width(Length::Fixed(ALBUM_TILE_W))
                    .padding(6),
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

    let first_idx = idxs[0];
    let first = &state.tracks[first_idx];

    let year = first
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "-".into());
    let genre = first.genre.clone().unwrap_or_else(|| "-".into());

    let total_tracks = idxs.len();
    let total_minutes: u32 = idxs
        .iter()
        .filter_map(|&i| state.tracks[i].duration_ms)
        .sum::<u32>()
        / 1000
        / 60;

    let back_btn = button("← Back to albums")
        .on_press(Message::SetViewMode(super::super::state::ViewMode::Albums));

    let play_album_btn = button("Play Album").on_press(Message::PlayAlbum(key.clone()));

    let rep_id = first.id;
    let big_cover = rep_id
        .and_then(|id| state.cover_cache.get(&id))
        .map(|h| cover_thumb(Some(h), ALBUM_DETAIL_COVER))
        .unwrap_or_else(|| cover_thumb(None, ALBUM_DETAIL_COVER));

    // Clicking the album header re-selects the whole album
    let header_content = row![
        big_cover,
        column![
            text(key.album.clone()).size(30),
            text(key.album_artist.clone()).size(20),
            text(format!("{genre} • {year}")).size(14),
            text(format!("{total_tracks} tracks • {total_minutes} min")).size(13),
            play_album_btn,
        ]
        .spacing(8)
        .width(Length::Fill),
    ]
    .spacing(24)
    .align_y(Alignment::Center);

    let header = mouse_area(
        container(header_content)
            .width(Length::Fill)
            .padding([4, 0]),
    )
    .on_press(Message::AlbumHeaderPressed(key.clone()));

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

        let marker = if is_now_playing {
            "▶"
        } else if is_selected || is_primary {
            "●"
        } else {
            ""
        };

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(24.0)),
            text(n)
                .size(ROW_TEXT)
                .width(Length::Fixed(ALBUM_DETAIL_TRACK_W_NO)),
            column![text(title).size(ROW_TEXT), text(artist).size(12)]
                .spacing(2)
                .width(Length::Fill),
            text(dur)
                .size(ROW_TEXT)
                .width(Length::Fixed(ALBUM_DETAIL_TRACK_W_LEN)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::AlbumTrackPressed(key.clone(), id));

        list = list.push(row_widget);
    }

    let tracks_panel = scrollable(list).height(Length::Fill);

    column![back_btn, header, tracks_panel].spacing(18)
}
