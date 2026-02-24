//! gui/update/scan.rs
use iced::Task;
use std::path::PathBuf;

use crate::core;

use super::super::state::{Message, Sonora, TEST_ROOT};
use super::selection::clear_selection_and_inspector;
use super::util::spawn_blocking;

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
        spawn_blocking(move || core::scan_and_read_roots(&roots_to_scan)),
        Message::ScanFinished,
    )
}

pub(crate) fn scan_finished(
    state: &mut Sonora,
    result: Result<(Vec<crate::core::types::TrackRow>, usize), String>,
) -> Task<Message> {
    state.scanning = false;

    match result {
        Ok((mut rows, tag_failures)) => {
            rows.sort_by(|a, b| a.path.cmp(&b.path));

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

            // After rescanning, any previous selection is invalid.
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
