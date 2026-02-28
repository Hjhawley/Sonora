//! gui/update/selection.rs
//!
//! Selection + view-mode transitions.
//!
//! - All selection is keyed by `TrackId` (stable), not `Vec` indices.
//! - Album expansion uses cached `state.album_groups` (no per-click O(n) scan).
//!
//! Cover art cache is keyed by `TrackId`.

use iced::Task;
use std::path::{Path, PathBuf};

use super::super::state::{AlbumKey, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_selection};
use super::util::spawn_blocking;
use crate::core::types::TrackId;

pub(crate) fn set_view_mode(state: &mut Sonora, mode: ViewMode) -> Task<Message> {
    state.view_mode = mode;

    state.selected_track = None;
    state.selected_tracks.clear();
    state.last_clicked_track = None;
    state.selected_album = None;

    clear_inspector(state);
    Task::none()
}

pub(crate) fn select_album(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    if state.view_mode != ViewMode::Albums {
        state.view_mode = ViewMode::Albums;
    }

    // Toggle collapse
    if state.selected_album.as_ref() == Some(&key) {
        clear_selection_and_inspector(state);
        return Task::none();
    }

    state.selected_album = Some(key.clone());
    state.selected_tracks.clear();

    // Pull ids from cache (fast).
    if let Some(ids) = state.album_groups.get(&key) {
        for &id in ids {
            state.selected_tracks.insert(id);
        }
    }

    // Choose a stable “primary” (BTreeSet keeps a stable order by id)
    state.selected_track = state.selected_tracks.iter().next().copied();
    state.last_clicked_track = state.selected_track;

    if state.selected_track.is_some() {
        load_inspector_from_selection(state);
    } else {
        clear_inspector(state);
        return Task::none();
    }

    // Kick off lazy cover load for the primary track (drives album row + big cover)
    let primary_id = state.selected_track.unwrap();
    maybe_load_cover_for_track(state, primary_id)
}

pub(crate) fn select_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    // If the id doesn't exist in the current list, ignore.
    let Some(idx) = state.index_of_id(id) else {
        return Task::none();
    };

    // In Album view:
    // - Clicking a track in the currently expanded album should NOT collapse the album.
    // - Clicking a track outside that album can collapse it.
    if state.view_mode == ViewMode::Albums {
        let clicked_key = album_key_for_index(state, idx);

        let keep_album_open = state.selected_album.as_ref().is_some_and(|k| {
            k.album_artist == clicked_key.album_artist && k.album == clicked_key.album
        });

        if !keep_album_open {
            state.selected_album = None;
        }
    } else {
        state.selected_album = None;
    }

    // Plain click: replace selection with this single track id.
    state.selected_tracks.clear();
    state.selected_tracks.insert(id);
    state.selected_track = Some(id);
    state.last_clicked_track = Some(id);

    load_inspector_from_selection(state);

    maybe_load_cover_for_track(state, id)
}

pub(crate) fn cover_loaded(
    state: &mut Sonora,
    id: TrackId,
    handle: Option<iced::widget::image::Handle>,
) -> Task<Message> {
    if let Some(h) = handle {
        state.cover_cache.insert(id, h);
    } else {
        state.cover_cache.remove(&id);
    }
    Task::none()
}

// Helpers

fn album_key_for_index(state: &Sonora, idx: usize) -> AlbumKey {
    let t = &state.tracks[idx];

    let album_artist = t
        .album_artist
        .clone()
        .or_else(|| t.artist.clone())
        .unwrap_or_else(|| "Unknown Artist".to_string());

    let album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    AlbumKey {
        album_artist,
        album,
    }
}

fn maybe_load_cover_for_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    // If we already have it, bail.
    if state.cover_cache.contains_key(&id) {
        return Task::none();
    }

    // Find the track to get the path.
    let Some(track) = state.track_by_id(id) else {
        return Task::none();
    };

    let path: PathBuf = track.path.clone();

    Task::perform(
        spawn_blocking(move || load_cover_handle_from_path(&path)),
        move |handle| Message::CoverLoaded(id, handle),
    )
}

fn load_cover_handle_from_path(path: &Path) -> Option<iced::widget::image::Handle> {
    let (bytes, _mime) = crate::core::tags::read_embedded_art(path).ok()??;
    Some(iced::widget::image::Handle::from_bytes(bytes))
}

pub(crate) fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_tracks.clear();
    state.last_clicked_track = None;
    state.selected_album = None;

    clear_inspector(state);
}
