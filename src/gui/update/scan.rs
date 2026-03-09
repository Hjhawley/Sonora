//! gui/update/scan.rs
//! Scan lifecycle + async boundary + selection reset.
//!
//! Scan pipeline:
//!   core::scan_paths(roots) -> Vec<PathBuf>
//!   SQLite upsert of discovered paths -> stable TrackIds
//!   read tags into TrackRows and attach DB-backed ids

use iced::Task;
use std::path::PathBuf;

use crate::core;

use super::super::state::{Message, Sonora, TEST_ROOT};
use super::selection::clear_selection_and_inspector;
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

    let roots_to_scan: Vec<PathBuf> = if state.roots.is_empty() {
        vec![PathBuf::from(TEST_ROOT)]
    } else {
        state.roots.clone()
    };

    Task::perform(
        spawn_blocking(move || {
            // discover paths
            let paths = core::scan_paths(&roots_to_scan)?;

            // get stable TrackIds
            let db_path = core::db::default_db_path()?;
            let mut db = core::db::Db::open(&db_path)?;
            let id_paths = db.upsert_paths(&paths)?;

            // read tags and attach ids
            let mut rows: Vec<TrackRow> = Vec::with_capacity(id_paths.len());
            let mut failures: usize = 0;

            for (id, path) in id_paths {
                let (mut row, failed) = core::tags::read_track_row(path);

                if failed {
                    failures += 1;
                }

                row.id = Some(id);
                rows.push(row);
            }

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

            // Rebuild id->index and album grouping caches for the new library.
            state.rebuild_library_caches();

            // New library = old ids/selection are invalid.
            clear_selection_and_inspector(state);
        }
        Err(e) => {
            // Keep previous tracks; just report error.
            state.status = format!("Scan error: {e}");
            clear_selection_and_inspector(state);
        }
    }

    Task::none()
}
