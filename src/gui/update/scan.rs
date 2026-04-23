//! gui/update/scan.rs
//!
//! GUI-side scan lifecycle.
//! This module starts the background scan task, maps GUI scope to core load scope,
//! and applies the finished scan result back into Sonora state.

use iced::Task;
use std::path::PathBuf;

use crate::core;
use crate::core::types::TrackRow;

use super::super::state::{LibraryScope, Message, Sonora, ViewMode};
use super::selection::{clear_selection_and_inspector, preload_album_covers};
use super::util::spawn_blocking;

pub(crate) fn scan_library(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    let roots_to_scan: Vec<PathBuf> = state.roots.clone();
    if roots_to_scan.is_empty() {
        state.status = "No library folders saved. Add a folder first.".to_string();
        return Task::none();
    }

    state.scanning = true;
    state.status = format!(
        "Scanning {} folder{}...",
        roots_to_scan.len(),
        if roots_to_scan.len() == 1 { "" } else { "s" }
    );

    clear_selection_and_inspector(state);

    let scope = match state.library_scope {
        LibraryScope::Library => core::LoadScope::Visible,
        LibraryScope::Hidden => core::LoadScope::Hidden,
        LibraryScope::Missing => core::LoadScope::Missing,
    };

    Task::perform(
        spawn_blocking(move || core::run_scan_for_scope(&roots_to_scan, scope)),
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
                    "Loaded {} tracks ({} tag refresh failures)",
                    rows.len(),
                    tag_failures
                )
            };

            state.tracks = rows;
            state.rebuild_library_caches();
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
