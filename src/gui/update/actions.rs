//! gui/update/actions.rs
//! DB-backed actions applied to the current selection.
//!
//! - Hidden/unhide only toggle Sonora visibility state.
//! - Delete-from-Sonora only removes missing rows from Sonora's DB.
//! - Each action reloads the active scope afterward.

use iced::Task;

use crate::core;
use crate::core::types::TrackId;

use super::super::state::{LibraryScope, Message, Sonora};
use super::scope::load_scope_tracks;
use super::selection::clear_selection_and_inspector;
use super::util::spawn_blocking;

pub(crate) fn hide_selected(state: &mut Sonora) -> Task<Message> {
    if state.library_scope != LibraryScope::Library {
        state.status = "Only library tracks can be hidden.".to_string();
        return Task::none();
    }

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
    if state.library_scope != LibraryScope::Hidden {
        state.status = "Only hidden tracks can be unhidden.".to_string();
        return Task::none();
    }

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

pub(crate) fn delete_selected_from_sonora(state: &mut Sonora) -> Task<Message> {
    if state.library_scope != LibraryScope::Missing {
        state.status = "Delete from Sonora is only available in Missing view.".to_string();
        return Task::none();
    }

    let ids = selected_ids(state);
    if ids.is_empty() {
        state.status = "Select one or more missing tracks first.".to_string();
        return Task::none();
    }

    state.status = if ids.len() == 1 {
        "Deleting missing track from Sonora...".to_string()
    } else {
        format!("Deleting {} missing tracks from Sonora...", ids.len())
    };

    let scope = state.library_scope;

    clear_selection_and_inspector(state);

    Task::perform(
        spawn_blocking(move || {
            let db_path = core::db::default_db_path()?;
            let db = core::db::Db::open(&db_path)?;

            for id in ids {
                db.delete_track(id)?;
            }

            load_scope_tracks(scope)
        }),
        Message::ScopeLoaded,
    )
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
