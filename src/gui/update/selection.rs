//! gui/update/selection.rs
use iced::Task;
use std::path::PathBuf;

use super::super::state::{AlbumKey, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_track};
use super::util::spawn_blocking;

pub(crate) fn set_view_mode(state: &mut Sonora, mode: ViewMode) -> Task<Message> {
    state.view_mode = mode;

    // Switching views should feel predictable.
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
        state.selected_album = None;

        state.selected_track = None;
        state.selected_tracks.clear();
        state.last_clicked_track = None;

        clear_inspector(state);
        return Task::none();
    }

    // Expanding selects the album AND selects all tracks in that album
    state.selected_album = Some(key.clone());
    state.selected_tracks.clear();

    for (i, t) in state.tracks.iter().enumerate() {
        let aa = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        if aa == key.album_artist && album == key.album {
            state.selected_tracks.insert(i);
        }
    }

    // Choose a stable “primary”
    state.selected_track = state.selected_tracks.iter().next().copied();
    state.last_clicked_track = state.selected_track;

    if state.selected_track.is_some() {
        load_inspector_from_track(state);
    } else {
        clear_inspector(state);
        return Task::none();
    }

    // Kick off lazy cover load for the primary track (drives album row + big cover)
    let primary_idx = state.selected_track.unwrap();
    maybe_load_cover_for_track(state, primary_idx)
}

pub(crate) fn select_track(state: &mut Sonora, index: usize) -> Task<Message> {
    if index >= state.tracks.len() {
        return Task::none();
    }

    // Clicking a track exits “album selects all” mode
    state.selected_album = None;

    // Plain click: replace selection with this single track
    state.selected_tracks.clear();
    state.selected_tracks.insert(index);
    state.selected_track = Some(index);
    state.last_clicked_track = Some(index);

    load_inspector_from_track(state);

    // Optional: also lazy-load cover for track view selections
    maybe_load_cover_for_track(state, index)
}

pub(crate) fn cover_loaded(
    state: &mut Sonora,
    index: usize,
    handle: Option<iced::widget::image::Handle>,
) -> Task<Message> {
    if let Some(h) = handle {
        state.cover_cache.insert(index, h);
    } else {
        // No art found: ensure we don't keep a stale handle
        state.cover_cache.remove(&index);
    }
    Task::none()
}

// Helpers

fn maybe_load_cover_for_track(state: &mut Sonora, index: usize) -> Task<Message> {
    if index >= state.tracks.len() {
        return Task::none();
    }
    if state.cover_cache.contains_key(&index) {
        return Task::none();
    }

    let path: PathBuf = state.tracks[index].path.clone();

    Task::perform(
        spawn_blocking(move || load_cover_handle_from_path(&path)),
        move |handle| Message::CoverLoaded(index, handle),
    )
}

/// Reads the first embedded ID3 picture (APIC/PIC) and returns an Iced Handle.
/// If no embedded art exists (or read fails), returns None.
fn load_cover_handle_from_path(path: &std::path::Path) -> Option<iced::widget::image::Handle> {
    let (bytes, _mime) = crate::core::tags::read_embedded_art(path).ok()??;
    Some(iced::widget::image::Handle::from_bytes(bytes))
}

// Helpers (state mutation)

pub(crate) fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_tracks.clear();
    state.last_clicked_track = None;
    state.selected_album = None;

    clear_inspector(state);
}
