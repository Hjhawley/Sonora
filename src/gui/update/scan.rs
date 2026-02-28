//! gui/update/scan.rs
//! Scan lifecycle + async boundary + selection reset.
//!
//! - Use the explicit core scan pipeline boundary:
//!   (A) core::scan_paths(roots) -> Vec<PathBuf>
//!   (B) core::read_tracks(paths) -> (Vec<TrackRow>, failures)
//!
//! Still no SQLite:
//! - We assign temporary TrackId values here so the GUI can operate id-first.
//! - Once SQLite lands, this becomes "load tracks from DB" instead.

use iced::Task;
use std::path::PathBuf;

use crate::core;

use super::super::state::{Message, Sonora, TEST_ROOT};
use super::selection::clear_selection_and_inspector;
use super::util::spawn_blocking;
use crate::core::types::{TrackId, TrackRow};

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
            // Stage A: discover paths (dedup + sorted in core)
            let paths = core::scan_paths(&roots_to_scan)?;
            // Stage B: read tags into TrackRows (non-fatal per-file)
            let (rows, failures) = core::read_tracks(paths);
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
        Ok((mut rows, tag_failures)) => {
            // Ensure every row has a TrackId (temporary, per-scan).
            assign_temp_ids_if_missing(&mut rows);

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

fn assign_temp_ids_if_missing(rows: &mut [TrackRow]) {
    // Deterministic and stable within a scan result.
    // Not stable across rescans (that’s what SQLite will fix).
    // TrackId is currently a *type alias* (not a newtype),
    // so assign by casting, not `TrackId(n)`.

    let mut next: u64 = 1;

    for r in rows.iter_mut() {
        if r.id.is_none() {
            r.id = Some(next as TrackId);
            next += 1;
        }
    }
}
