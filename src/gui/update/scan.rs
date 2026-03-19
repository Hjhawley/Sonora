//! gui/update/scan.rs
//! Scan lifecycle + async boundary + selection reset.
//!
//! Scan pipeline:
//! - discover current filesystem paths + file facts
//! - reconcile them into SQLite, updating 'present', 'mtime', 'size'
//! - reload the currently active scope from DB
//! - hydrate TrackRows from those DB-backed ids/paths
//!
//! Important:
//! The DB is the source of truth after reconciliation.
//! UI should not be built directly from the raw discovered set.

use iced::Task;
use std::path::PathBuf;

use crate::core;

/* use super::super::state::{LibraryScope, Message, Sonora, TEST_ROOT, ViewMode}; */
use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::selection::{clear_selection_and_inspector, preload_album_covers};
use super::util::spawn_blocking;
use crate::core::types::TrackRow;

pub(crate) fn scan_library(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    state.scanning = true;
    state.status = "Scanning...".to_string();

    // Selection becomes invalid once new results arrive, but keeping tracks visible
    // during scan is nicer UX (and avoids an empty UI if scan fails).
    clear_selection_and_inspector(state);

    /* let roots_to_scan: Vec<PathBuf> = if state.roots.is_empty() {
        vec![PathBuf::from(TEST_ROOT)]
    } else {
        state.roots.clone()
    }; */

    let roots_to_scan: Vec<PathBuf> = state.roots.clone();

    let scope = state.library_scope;

    Task::perform(
        spawn_blocking(move || {
            let discovered = core::scan_paths(&roots_to_scan)?;

            let db_path = core::db::default_db_path()?;
            let mut db = core::db::Db::open(&db_path)?;

            // Reconcile filesystem facts into DB truth.
            db.upsert_discovered(&discovered)?;

            let id_paths = match scope {
                LibraryScope::Library => db.load_visible_paths()?,
                LibraryScope::Hidden => db.load_hidden_paths()?,
                LibraryScope::Missing => db.load_missing_paths()?,
            };

            let (rows, failures) = core::hydrate_tracks(id_paths);
            Ok((rows, failures))
        }),
        Message::ScanFinished,
    )
}

pub(crate) fn scan_finished(
    state: &mut Sonora,
    result: Result<(Vec<TrackRow>, usize), String>,
) -> Task<Message> {
    state.scanning = false;

    match result {
        Ok((rows, tag_failures)) => {
            state.status = if tag_failures == 0 {
                format!("Loaded {} tracks", rows.len())
            } else {
                format!(
                    "Loaded {} tracks ({} tag read failures)",
                    rows.len(),
                    tag_failures
                )
            };

            state.tracks = rows;
            state.rebuild_library_derived_state();
            clear_selection_and_inspector(state);

            if state.view_mode == ViewMode::Albums {
                return preload_album_covers(state);
            }
        }
        Err(e) => {
            state.status = format!("Scan error: {e}");
            clear_selection_and_inspector(state);
        }
    }

    Task::none()
}
