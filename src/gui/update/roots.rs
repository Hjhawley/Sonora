//! gui/update/roots.rs
//!
//! Root-folder management.
//! Roots are DB-backed persistent library configuration.
//! The GUI keeps an in-memory copy in 'state.roots', but add/remove operations
//! write through to SQLite immediately.

use iced::Task;
use std::path::{Path, PathBuf};

use crate::core;

use super::super::state::{LibraryScope, Message, Sonora};
use super::selection::clear_selection_and_inspector;

pub(crate) fn root_input_changed(state: &mut Sonora, s: String) -> Task<Message> {
    state.root_input = s;
    Task::none()
}

pub(crate) fn add_root_pressed(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    let input = state.root_input.trim();
    if input.is_empty() {
        return Task::none();
    }

    let p = PathBuf::from(input);

    // Validate: user must add an existing directory.
    if !Path::new(input).is_dir() {
        state.status = format!("Not a folder: {}", p.display());
        return Task::none();
    }

    // Avoid exact duplicates in current UI state.
    if state.roots.contains(&p) {
        state.status = format!("Already added: {}", p.display());
        state.root_input.clear();
        return Task::none();
    }

    let result = (|| -> Result<(), String> {
        let db_path = core::db::default_db_path()?;
        let db = core::db::Db::open(&db_path)?;
        db.add_root(&p)?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            state.roots.push(p.clone());
            state.roots.sort();
            state.root_input.clear();
            state.status = format!("Added folder: {}", p.display());
        }
        Err(e) => {
            state.status = format!("Could not add folder: {e}");
        }
    }

    Task::none()
}

pub(crate) fn remove_root(state: &mut Sonora, i: usize) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if i >= state.roots.len() {
        return Task::none();
    }

    let removed = state.roots[i].clone();

    let result = (|| -> Result<usize, String> {
        let db_path = core::db::default_db_path()?;
        let db = core::db::Db::open(&db_path)?;

        db.remove_root(&removed)?;
        let remaining_roots = db.load_roots()?;
        let deleted = db.delete_uncovered_tracks_under_root(&removed, &remaining_roots)?;

        Ok(deleted)
    })();

    match result {
        Ok(deleted_count) => {
            state.roots.remove(i);

            let reload = match state.library_scope {
                LibraryScope::Library => core::load_visible_tracks_from_db(),
                LibraryScope::Hidden => core::load_hidden_tracks_from_db(),
                LibraryScope::Missing => core::load_missing_tracks_from_db(),
            };

            match reload {
                Ok((rows, failures)) => {
                    state.tracks = rows;
                    state.rebuild_library_derived_state();
                    clear_selection_and_inspector(state);

                    state.status = if failures == 0 {
                        format!(
                            "Removed folder: {} (removed {} uncovered track record{})",
                            removed.display(),
                            deleted_count,
                            if deleted_count == 1 { "" } else { "s" }
                        )
                    } else {
                        format!(
                            "Removed folder: {} (removed {} uncovered track record{}, reloaded with {} tag read failures)",
                            removed.display(),
                            deleted_count,
                            if deleted_count == 1 { "" } else { "s" },
                            failures
                        )
                    };
                }
                Err(e) => {
                    clear_selection_and_inspector(state);
                    state.status = format!(
                        "Removed folder: {} (removed {} uncovered track record{}), but reload failed: {e}",
                        removed.display(),
                        deleted_count,
                        if deleted_count == 1 { "" } else { "s" }
                    );
                }
            }
        }
        Err(e) => {
            state.status = format!("Could not remove folder: {e}");
        }
    }

    Task::none()
}
