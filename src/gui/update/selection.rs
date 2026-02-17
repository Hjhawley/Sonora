use iced::Task;

use super::super::state::{AlbumKey, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_track};

pub(crate) fn set_view_mode(state: &mut Sonora, mode: ViewMode) -> Task<Message> {
    state.view_mode = mode;

    // Switching views should feel predictable.
    state.selected_track = None;
    clear_inspector(state);

    if mode == ViewMode::Tracks {
        state.selected_album = None;
    }

    Task::none()
}

pub(crate) fn select_album(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    if state.view_mode != ViewMode::Albums {
        state.view_mode = ViewMode::Albums;
    }

    // toggle collapse
    if state.selected_album.as_ref() == Some(&key) {
        state.selected_album = None;
    } else {
        state.selected_album = Some(key);
    }

    state.selected_track = None;
    clear_inspector(state);
    Task::none()
}

pub(crate) fn select_track(state: &mut Sonora, i: usize) -> Task<Message> {
    if i < state.tracks.len() {
        state.selected_track = Some(i);
        load_inspector_from_track(state);
    }
    Task::none()
}

// --------------------
// Helpers (state mutation)
// --------------------

pub(crate) fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_album = None;
    clear_inspector(state);
}
