//! gui/update/scope.rs
//!
//! Library-scope and view-mode transitions.
//!
//! - Scope changes asynchronously reload rows from the DB-backed library cache.
//! - View-mode changes reset incompatible selection state.
//! - Album View may trigger representative cover preloading.

use iced::Task;

use crate::core;
use crate::core::types::TrackRow;

use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::selection::{clear_selection_and_inspector, preload_album_covers};
use super::util::spawn_blocking;

pub(crate) fn set_library_scope(state: &mut Sonora, scope: LibraryScope) -> Task<Message> {
    if state.library_scope == scope {
        return Task::none();
    }

    state.library_scope = scope;
    state.status = match scope {
        LibraryScope::Library => "Loading library...".to_string(),
        LibraryScope::Hidden => "Loading hidden tracks...".to_string(),
        LibraryScope::Missing => "Loading missing tracks...".to_string(),
    };

    clear_selection_and_inspector(state);

    Task::perform(
        spawn_blocking(move || load_scope_tracks(scope)),
        Message::ScopeLoaded,
    )
}

pub(crate) fn scope_loaded(
    state: &mut Sonora,
    result: Result<(LibraryScope, Vec<TrackRow>, usize), String>,
) -> Task<Message> {
    match result {
        Ok((scope, rows, failures)) => {
            state.library_scope = scope;
            state.tracks = rows;
            state.rebuild_library_caches();
            clear_selection_and_inspector(state);

            state.status = match (scope, state.tracks.len(), failures) {
                (LibraryScope::Library, 0, _) => "Library is empty.".to_string(),
                (LibraryScope::Hidden, 0, _) => "No hidden tracks.".to_string(),
                (LibraryScope::Missing, 0, _) => "No missing tracks.".to_string(),

                (LibraryScope::Library, n, 0) => format!("Loaded {n} library tracks."),
                (LibraryScope::Hidden, n, 0) => format!("Loaded {n} hidden tracks."),
                (LibraryScope::Missing, n, 0) => format!("Loaded {n} missing tracks."),

                (LibraryScope::Library, n, f) => {
                    format!("Loaded {n} library tracks ({f} tag read failures).")
                }
                (LibraryScope::Hidden, n, f) => {
                    format!("Loaded {n} hidden tracks ({f} tag read failures).")
                }
                (LibraryScope::Missing, n, f) => {
                    format!("Loaded {n} missing tracks ({f} tag read failures).")
                }
            };

            if state.view_mode == ViewMode::Albums {
                return preload_album_covers(state);
            }
        }
        Err(e) => {
            state.status = format!("Load failed: {e}");
            clear_selection_and_inspector(state);
        }
    }

    Task::none()
}

pub(crate) fn set_view_mode(state: &mut Sonora, mode: ViewMode) -> Task<Message> {
    let was_albums = state.view_mode == ViewMode::Albums;

    state.view_mode = mode;
    state.last_album_press = None;

    if mode == ViewMode::Tracks {
        state.selected_album = None;
    }

    if mode == ViewMode::Albums && !was_albums {
        state.selected_album = None;
        state.selected_track = None;
        state.selected_tracks.clear();
        state.selection_anchor = None;
        state.last_clicked_track = None;
        super::inspector::clear_inspector(state);
        return preload_album_covers(state);
    }

    state.selected_track = None;
    state.selected_tracks.clear();
    state.selection_anchor = None;
    state.last_clicked_track = None;
    state.selected_album = None;

    super::inspector::clear_inspector(state);

    if mode == ViewMode::Albums {
        return preload_album_covers(state);
    }

    Task::none()
}

pub(crate) fn load_scope_tracks(
    scope: LibraryScope,
) -> Result<(LibraryScope, Vec<TrackRow>, usize), String> {
    let result = match scope {
        LibraryScope::Library => core::load_visible_tracks_from_db(),
        LibraryScope::Hidden => core::load_hidden_tracks_from_db(),
        LibraryScope::Missing => core::load_missing_tracks_from_db(),
    }?;

    Ok((scope, result.0, result.1))
}
