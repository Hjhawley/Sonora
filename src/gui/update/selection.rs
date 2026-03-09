//! gui/update/selection.rs
//!
//! Selection + scope + view-mode transitions.
//!
//! - All selection is keyed by `TrackId` (stable), not `Vec` indices.
//! - Album View is now:
//!   - grid when `selected_album == None`
//!   - album detail screen when `selected_album == Some(...)`
//! - Hidden/unhide is DB-backed and never touches the underlying file.

use iced::Task;
use std::path::{Path, PathBuf};

use super::super::state::{AlbumKey, LibraryScope, Message, Sonora, ViewMode};
use super::inspector::{clear_inspector, load_inspector_from_selection};
use super::util::spawn_blocking;
use crate::core;
use crate::core::types::{TrackId, TrackRow};

pub(crate) fn set_library_scope(state: &mut Sonora, scope: LibraryScope) -> Task<Message> {
    if state.library_scope == scope {
        return Task::none();
    }

    state.library_scope = scope;
    state.status = match scope {
        LibraryScope::Library => "Loading library...".to_string(),
        LibraryScope::Hidden => "Loading hidden tracks...".to_string(),
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
                (LibraryScope::Library, n, 0) => format!("Loaded {n} library tracks."),
                (LibraryScope::Hidden, n, 0) => format!("Loaded {n} hidden tracks."),
                (LibraryScope::Library, n, f) => {
                    format!("Loaded {n} library tracks ({f} tag read failures).")
                }
                (LibraryScope::Hidden, n, f) => {
                    format!("Loaded {n} hidden tracks ({f} tag read failures).")
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
    state.view_mode = mode;

    state.selected_track = None;
    state.selected_tracks.clear();
    state.last_clicked_track = None;
    state.selected_album = None;

    clear_inspector(state);

    if mode == ViewMode::Albums {
        return preload_album_covers(state);
    }

    Task::none()
}

pub(crate) fn select_album(state: &mut Sonora, key: AlbumKey) -> Task<Message> {
    if state.view_mode != ViewMode::Albums {
        state.view_mode = ViewMode::Albums;
    }

    // Clicking the same album while already in detail acts like "Back".
    if state.selected_album.as_ref() == Some(&key) {
        clear_selection_and_inspector(state);
        return Task::none();
    }

    state.selected_album = Some(key.clone());
    state.selected_tracks.clear();

    if let Some(ids) = state.album_groups.get(&key) {
        for &id in ids {
            state.selected_tracks.insert(id);
        }
    }

    // Primary = first track in album, stable by TrackId ordering in the set.
    state.selected_track = state.selected_tracks.iter().next().copied();
    state.last_clicked_track = state.selected_track;

    if state.selected_track.is_some() {
        load_inspector_from_selection(state);
    } else {
        clear_inspector(state);
        return Task::none();
    }

    let primary_id = state.selected_track.unwrap();
    maybe_load_cover_for_track(state, primary_id)
}

pub(crate) fn select_track(state: &mut Sonora, id: TrackId) -> Task<Message> {
    let Some(idx) = state.index_of_id(id) else {
        return Task::none();
    };

    // In Album detail, keep the detail screen open if the clicked track belongs to the open album.
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

    state.selected_tracks.clear();
    state.selected_tracks.insert(id);
    state.selected_track = Some(id);
    state.last_clicked_track = Some(id);

    load_inspector_from_selection(state);

    maybe_load_cover_for_track(state, id)
}

pub(crate) fn hide_selected(state: &mut Sonora) -> Task<Message> {
    let ids = selected_ids(state);
    if ids.is_empty() {
        state.status = "Select one or more tracks first.".to_string();
        return Task::none();
    }

    state.status = if ids.len() == 1 {
        "Hiding track from Sonora...".to_string()
    } else {
        format!("Hiding {} tracks from Sonora...", ids.len())
    };

    let scope = state.library_scope;

    clear_selection_and_inspector(state);

    Task::perform(
        spawn_blocking(move || {
            let db_path = core::db::default_db_path()?;
            let db = core::db::Db::open(&db_path)?;

            for id in ids {
                db.set_hidden(id, true)?;
            }

            load_scope_tracks(scope)
        }),
        Message::ScopeLoaded,
    )
}

pub(crate) fn unhide_selected(state: &mut Sonora) -> Task<Message> {
    let ids = selected_ids(state);
    if ids.is_empty() {
        state.status = "Select one or more hidden tracks first.".to_string();
        return Task::none();
    }

    state.status = if ids.len() == 1 {
        "Unhiding track...".to_string()
    } else {
        format!("Unhiding {} tracks...", ids.len())
    };

    let scope = state.library_scope;

    clear_selection_and_inspector(state);

    Task::perform(
        spawn_blocking(move || {
            let db_path = core::db::default_db_path()?;
            let db = core::db::Db::open(&db_path)?;

            for id in ids {
                db.set_hidden(id, false)?;
            }

            load_scope_tracks(scope)
        }),
        Message::ScopeLoaded,
    )
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

/// Preload representative cover art for every album currently in the dataset.
/// This makes the album grid feel alive immediately, instead of only loading after click.
pub(crate) fn preload_album_covers(state: &mut Sonora) -> Task<Message> {
    let rep_ids: Vec<TrackId> = state
        .album_groups
        .values()
        .filter_map(|ids| ids.first().copied())
        .collect();

    let mut tasks: Vec<Task<Message>> = Vec::new();
    for id in rep_ids {
        tasks.push(maybe_load_cover_for_track(state, id));
    }

    Task::batch(tasks)
}

fn load_scope_tracks(scope: LibraryScope) -> Result<(LibraryScope, Vec<TrackRow>, usize), String> {
    let result = match scope {
        LibraryScope::Library => core::load_visible_tracks_from_db(),
        LibraryScope::Hidden => core::load_hidden_tracks_from_db(),
    }?;

    Ok((scope, result.0, result.1))
}

fn selected_ids(state: &Sonora) -> Vec<TrackId> {
    let mut ids: Vec<TrackId> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(id) = state.selected_track {
        vec![id]
    } else {
        vec![]
    };

    ids.sort_unstable();
    ids.dedup();
    ids
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
    if state.cover_cache.contains_key(&id) {
        return Task::none();
    }

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
