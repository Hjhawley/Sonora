use iced::Task;

use super::super::state::{AlbumKey, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_track};

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
    }

    Task::none()
}

pub(crate) fn select_track(state: &mut Sonora, index: usize) -> Task<Message> {
    if index >= state.tracks.len() {
        return Task::none();
    }

    // Clicking a track exits “album selects all” mode
    state.selected_album = None;

    // NOTE:
    // We do NOT have ctrl/shift wired yet because Message::SelectTrack only carries `usize`.
    // For now, we keep behavior simple and predictable:
    // - Plain click: replace selection with this single track
    // (We'll upgrade this once we add modifier messages / event subscription.)

    state.selected_tracks.clear();
    state.selected_tracks.insert(index);
    state.selected_track = Some(index);
    state.last_clicked_track = Some(index);

    load_inspector_from_track(state);
    Task::none()
}

// --------------------
// Helpers (state mutation)
// --------------------

pub(crate) fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_tracks.clear();
    state.last_clicked_track = None;
    state.selected_album = None;

    clear_inspector(state);
}
