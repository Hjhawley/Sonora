//! gui/update/scan.rs
//! Scan lifecycle + async boundary + selection reset.
//!
//! pipeline:
//! - discover filesystem facts
//! - reconcile present/missing + detect changed files
//! - hydrate only changed/new files
//! - persist refreshed metadata into DB
//! - reload current scope directly from DB

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

    let scope = state.library_scope;

    Task::perform(
        spawn_blocking(move || {
            let discovered = core::scan_paths(&roots_to_scan)?;

            let db_path = core::db::default_db_path()?;
            let mut db = core::db::Db::open(&db_path)?;

            let changed_id_paths = db.upsert_discovered(&roots_to_scan, &discovered)?;

            let tag_failures = if changed_id_paths.is_empty() {
                0usize
            } else {
                let (rows, failures) = core::hydrate_tracks(changed_id_paths);
                db.upsert_track_rows_metadata(&rows)?;
                failures
            };

            let rows = match scope {
                LibraryScope::Library => db.load_visible_tracks()?,
                LibraryScope::Hidden => db.load_hidden_tracks()?,
                LibraryScope::Missing => db.load_missing_tracks()?,
            };

            Ok((rows, tag_failures))
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
