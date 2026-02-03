//! Update logic ("the brain").
//!
//! This file contains:
//! - the main 'update()' function (handles messages)
//! - small state-mutation helpers (load/clear inspector)
//!
//! Rule of thumb:
//! - If it *changes* state → it belongs here (not in view.rs)

use iced::Task;
use iced::futures::channel::oneshot;
use std::path::{Path, PathBuf};

use crate::core;
use crate::core::types::TrackRow;

use super::state::{Message, Sonora, TEST_ROOT, ViewMode};
use super::util::{clean_optional_string, filename_stem, parse_optional_i32, parse_optional_u32};

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        // Roots (folders)
        Message::RootInputChanged(s) => {
            state.root_input = s;
            Task::none()
        }

        Message::AddRootPressed => {
            let input = state.root_input.trim();
            if input.is_empty() {
                return Task::none();
            }

            let p = PathBuf::from(input);

            // Don't let user add garbage paths
            if !Path::new(input).is_dir() {
                state.status = format!("Not a folder: {}", p.display());
                return Task::none();
            }

            // Don't add duplicates
            if state.roots.contains(&p) {
                state.status = format!("Already added: {}", p.display());
                state.root_input.clear();
                return Task::none();
            }

            state.roots.push(p.clone());
            state.root_input.clear();
            state.status = format!("Added folder: {}", p.display());

            Task::none()
        }

        Message::RemoveRoot(i) => {
            if i < state.roots.len() && !state.scanning {
                let removed = state.roots.remove(i);
                state.status = format!("Removed folder: {}", removed.display());
            }
            Task::none()
        }

        // Scan
        Message::ScanLibrary => {
            if state.scanning {
                return Task::none();
            }

            state.scanning = true;
            state.tracks.clear();
            state.status = "Scanning…".to_string();

            // If user hasn't added roots, scan ./test
            let roots_to_scan = if state.roots.is_empty() {
                vec![PathBuf::from(TEST_ROOT)]
            } else {
                state.roots.clone()
            };

            Task::perform(
                async move {
                    let (tx, rx) = oneshot::channel::<Result<(Vec<TrackRow>, usize), String>>();

                    std::thread::spawn(move || {
                        // This is the "heavy work" thread
                        let _ = tx.send(core::scan_and_read_roots(roots_to_scan));
                    });

                    rx.await
                        .map_err(|_| "Scan thread dropped without returning".to_string())?
                },
                Message::ScanFinished,
            )
        }

        Message::ScanFinished(result) => {
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

                    // After rescanning, user's selections may not make sense anymore.
                    state.selected_track = None;
                    state.selected_album = None;
                    clear_inspector(state);
                }
                Err(e) => {
                    state.status = format!("Error: {e}");
                    state.tracks.clear();
                }
            }

            Task::none()
        }

        // View mode
        Message::SetViewMode(mode) => {
            state.view_mode = mode;

            // Switching views should feel predictable:
            state.selected_track = None;
            clear_inspector(state);

            if mode == ViewMode::Tracks {
                state.selected_album = None;
            }

            Task::none()
        }

        // Album selection
        Message::SelectAlbum(key) => {
            state.selected_album = Some(key);
            state.selected_track = None; // selecting an album is not selecting a track
            clear_inspector(state);
            Task::none()
        }

        // Track selection
        Message::SelectTrack(i) => {
            if i < state.tracks.len() {
                state.selected_track = Some(i);
                load_inspector_from_track(state);
            }
            Task::none()
        }

        // Inspector typing
        Message::EditTitle(s) => {
            state.inspector.title = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditArtist(s) => {
            state.inspector.artist = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditAlbum(s) => {
            state.inspector.album = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditTrackNo(s) => {
            state.inspector.track_no = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditYear(s) => {
            state.inspector.year = s;
            state.inspector_dirty = true;
            Task::none()
        }

        // Save = currently only updates memory (not disk)
        Message::SaveInspectorToMemory => {
            let Some(i) = state.selected_track else {
                state.status = "Select a track first.".to_string();
                return Task::none();
            };

            if i >= state.tracks.len() {
                return Task::none();
            }

            // Parse numbers
            let track_no = parse_optional_u32(&state.inspector.track_no)
                .map_err(|_| "Track # must be a number".to_string());

            let year = parse_optional_i32(&state.inspector.year)
                .map_err(|_| "Year must be a number".to_string());

            // Borrow the errors without moving the Results
            let track_err = track_no.as_ref().err();
            let year_err = year.as_ref().err();

            if track_err.is_some() || year_err.is_some() {
                let mut msg = String::from("Not saved: ");

                if let Some(e) = track_err {
                    msg.push_str(e);
                }

                if track_err.is_some() && year_err.is_some() {
                    msg.push_str(" | ");
                }

                if let Some(e) = year_err {
                    msg.push_str(e);
                }

                state.status = msg;
                return Task::none();
            }

            // Now it's safe to unwrap because we know they're Ok(...)
            let track_no = track_no.unwrap();
            let year = year.unwrap();

            // Write values into the in-memory TrackRow
            let t = &mut state.tracks[i];

            t.title = clean_optional_string(&state.inspector.title);
            t.artist = clean_optional_string(&state.inspector.artist);
            t.album = clean_optional_string(&state.inspector.album);
            t.track_no = track_no;
            t.year = year;

            state.inspector_dirty = false;
            state.status = "Changes saved to memory, not written to files (yet)".to_string();

            Task::none()
        }

        Message::RevertInspector => {
            load_inspector_from_track(state);
            Task::none()
        }
    }
}

// Helpers (state mutation)
fn load_inspector_from_track(state: &mut Sonora) {
    let Some(i) = state.selected_track else {
        clear_inspector(state);
        return;
    };

    if i >= state.tracks.len() {
        clear_inspector(state);
        return;
    }

    let t = &state.tracks[i];

    state.inspector.title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    state.inspector.artist = t
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown Artist".to_string());
    state.inspector.album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    state.inspector.track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.year = t.year.map(|y| y.to_string()).unwrap_or_default();

    state.inspector_dirty = false;
}

fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
}
