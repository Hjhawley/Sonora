//! Update logic.
//! Mutates state in response to `Message` events.
//!
//! Notes:
//! - The UI thread should stay responsive.
//! - Potentially slow work (filesystem scan, tag writes) is done on a background thread.
//! - Background threads report back via `Message::*Finished` with a `Result`.

use iced::Task;
use iced::futures::channel::oneshot;
use std::path::{Path, PathBuf};

use crate::core;

use super::state::{InspectorField, Message, Sonora, TEST_ROOT, ViewMode};
use super::util::{filename_stem, parse_optional_i32, parse_optional_u32};

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        // --------------------
        // Roots
        // --------------------
        Message::RootInputChanged(s) => {
            state.root_input = s;
            Task::none()
        }

        Message::AddRootPressed => {
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

            // Avoid duplicates.
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
            if i < state.roots.len() && !state.scanning && !state.saving {
                let removed = state.roots.remove(i);
                state.status = format!("Removed folder: {}", removed.display());
            }
            Task::none()
        }

        // --------------------
        // Scan
        // --------------------
        Message::ScanLibrary => {
            if state.scanning || state.saving {
                return Task::none();
            }

            state.scanning = true;
            state.tracks.clear();
            state.status = "Scanning...".to_string();
            clear_selection_and_inspector(state);

            // If user hasn't added roots, scan ./test
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

                    // After rescanning, any previous selection is invalid.
                    clear_selection_and_inspector(state);
                }
                Err(e) => {
                    state.status = format!("Error: {e}");
                    state.tracks.clear();
                    clear_selection_and_inspector(state);
                }
            }

            Task::none()
        }

        // --------------------
        // View mode
        // --------------------
        Message::SetViewMode(mode) => {
            state.view_mode = mode;

            // Switching views should feel predictable.
            state.selected_track = None;
            clear_inspector(state);

            if mode == ViewMode::Tracks {
                state.selected_album = None;
            }

            Task::none()
        }

        // Album selection (toggle collapse)
        Message::SelectAlbum(key) => {
            if state.view_mode != ViewMode::Albums {
                state.view_mode = ViewMode::Albums;
            }

            if state.selected_album.as_ref() == Some(&key) {
                state.selected_album = None; // collapse
            } else {
                state.selected_album = Some(key); // expand
            }

            state.selected_track = None;
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

        // --------------------
        // Inspector toggles
        // --------------------
        Message::ToggleExtended(v) => {
            state.show_extended = v;
            Task::none()
        }

        // --------------------
        // Inspector typing (core + extended)
        // --------------------
        Message::InspectorChanged(field, value) => {
            set_inspector_field(state, field, value);
            state.inspector_dirty = true;
            Task::none()
        }

        // --------------------
        // Save to disk
        // --------------------
        Message::SaveInspectorToFile => {
            if state.scanning || state.saving {
                return Task::none();
            }

            if !state.inspector_dirty {
                state.status = "No changes to save.".to_string();
                return Task::none();
            }

            let Some(i) = state.selected_track else {
                state.status = "Select a track first.".to_string();
                return Task::none();
            };
            if i >= state.tracks.len() {
                state.status = "Invalid selection (rescan?).".to_string();
                return Task::none();
            }

            // Build the row we want to write (from inspector), with validation.
            let row_to_write = match build_row_from_inspector(state, i) {
                Ok(r) => r,
                Err(e) => {
                    state.status = e;
                    return Task::none();
                }
            };

            state.saving = true;
            state.status = "Writing tags to file...".to_string();
            let write_extended = state.show_extended;

            Task::perform(
                spawn_blocking(move || {
                    crate::core::tags::write_track_row(&row_to_write, write_extended).and_then(
                        |_| {
                            let (r, failed) =
                                crate::core::tags::read_track_row(row_to_write.path.clone());
                            if failed {
                                Err("Wrote tags, but failed to re-read them".to_string())
                            } else {
                                Ok(r)
                            }
                        },
                    )
                }),
                move |res| Message::SaveFinished(i, res),
            )
        }

        Message::SaveFinished(i, result) => {
            state.saving = false;

            match result {
                Ok(new_row) => {
                    if i < state.tracks.len() {
                        state.tracks[i] = new_row;
                        load_inspector_from_track(state);
                    }
                    state.inspector_dirty = false;
                    state.status = "Tags written to file.".to_string();
                }
                Err(e) => {
                    state.status = format!("Save failed: {e}");
                }
            }

            Task::none()
        }

        Message::RevertInspector => {
            load_inspector_from_track(state);
            Task::none()
        }
    }
}

/// Update a single inspector string field based on `InspectorField`.
fn set_inspector_field(state: &mut Sonora, field: InspectorField, value: String) {
    match field {
        // Core
        InspectorField::Title => state.inspector.title = value,
        InspectorField::Artist => state.inspector.artist = value,
        InspectorField::Album => state.inspector.album = value,
        InspectorField::AlbumArtist => state.inspector.album_artist = value,
        InspectorField::Composer => state.inspector.composer = value,

        InspectorField::TrackNo => state.inspector.track_no = value,
        InspectorField::TrackTotal => state.inspector.track_total = value,
        InspectorField::DiscNo => state.inspector.disc_no = value,
        InspectorField::DiscTotal => state.inspector.disc_total = value,

        InspectorField::Year => state.inspector.year = value,
        InspectorField::Date => state.inspector.date = value,
        InspectorField::Genre => state.inspector.genre = value,

        // Extended
        InspectorField::Grouping => state.inspector.grouping = value,
        InspectorField::Comment => state.inspector.comment = value,
        InspectorField::Lyrics => state.inspector.lyrics = value,
        InspectorField::Lyricist => state.inspector.lyricist = value,
        InspectorField::Conductor => state.inspector.conductor = value,
        InspectorField::Remixer => state.inspector.remixer = value,
        InspectorField::Publisher => state.inspector.publisher = value,
        InspectorField::Subtitle => state.inspector.subtitle = value,

        InspectorField::Bpm => state.inspector.bpm = value,
        InspectorField::Key => state.inspector.key = value,
        InspectorField::Mood => state.inspector.mood = value,
        InspectorField::Language => state.inspector.language = value,
        InspectorField::Isrc => state.inspector.isrc = value,
        InspectorField::EncoderSettings => state.inspector.encoder_settings = value,
        InspectorField::EncodedBy => state.inspector.encoded_by = value,
        InspectorField::Copyright => state.inspector.copyright = value,
    }
}

/// Run a blocking function on a background thread and await the result.
///
/// This is intentionally tiny: it avoids repeating the oneshot + thread boilerplate
/// for every “do work off-thread, then send Message::Finished(Result<...>)” case.
async fn spawn_blocking<T>(f: impl FnOnce() -> T + Send + 'static) -> T
where
    T: Send + 'static,
{
    let (tx, rx) = oneshot::channel::<T>();

    std::thread::spawn(move || {
        let _ = tx.send(f());
    });

    rx.await
        .expect("background worker dropped without returning")
}

/// Build the TrackRow to write by applying the inspector draft onto the currently selected row.
///
/// Semantics:
/// - Core fields are always applied.
/// - Extended fields are only applied if `state.show_extended == true`.
///   (If the user isn't showing them, we preserve existing values to avoid accidental deletion.)
/// - Empty/whitespace input becomes `None` (your tag writer treats `None` as “remove this tag”.)
fn build_row_from_inspector(
    state: &Sonora,
    i: usize,
) -> Result<crate::core::types::TrackRow, String> {
    let mut out = state
        .tracks
        .get(i)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    // Parse numeric fields (collect invalid labels; we don't auto-correct user input).
    let mut errs: Vec<&'static str> = Vec::new();

    let track_no = parse_optional_u32(&state.inspector.track_no)
        .inspect_err(|_| errs.push("Track #"))
        .ok()
        .flatten();

    let track_total = parse_optional_u32(&state.inspector.track_total)
        .inspect_err(|_| errs.push("Track total"))
        .ok()
        .flatten();

    let disc_no = parse_optional_u32(&state.inspector.disc_no)
        .inspect_err(|_| errs.push("Disc #"))
        .ok()
        .flatten();

    let disc_total = parse_optional_u32(&state.inspector.disc_total)
        .inspect_err(|_| errs.push("Disc total"))
        .ok()
        .flatten();

    let year = parse_optional_i32(&state.inspector.year)
        .inspect_err(|_| errs.push("Year"))
        .ok()
        .flatten();

    let bpm = if state.show_extended {
        parse_optional_u32(&state.inspector.bpm)
            .inspect_err(|_| errs.push("BPM"))
            .ok()
            .flatten()
    } else {
        out.bpm
    };

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    // Core (always applied)
    out.title = clean_opt(&state.inspector.title);
    out.artist = clean_opt(&state.inspector.artist);
    out.album = clean_opt(&state.inspector.album);
    out.album_artist = clean_opt(&state.inspector.album_artist);
    out.composer = clean_opt(&state.inspector.composer);

    out.track_no = track_no;
    out.track_total = track_total;
    out.disc_no = disc_no;
    out.disc_total = disc_total;

    out.year = year;
    out.date = clean_opt(&state.inspector.date);
    out.genre = clean_opt(&state.inspector.genre);

    // Extended (only if visible)
    if state.show_extended {
        out.grouping = clean_opt(&state.inspector.grouping);
        out.comment = clean_opt(&state.inspector.comment);
        out.lyrics = clean_opt(&state.inspector.lyrics);

        out.lyricist = clean_opt(&state.inspector.lyricist);
        out.conductor = clean_opt(&state.inspector.conductor);
        out.remixer = clean_opt(&state.inspector.remixer);
        out.publisher = clean_opt(&state.inspector.publisher);
        out.subtitle = clean_opt(&state.inspector.subtitle);

        out.bpm = bpm;
        out.key = clean_opt(&state.inspector.key);
        out.mood = clean_opt(&state.inspector.mood);
        out.language = clean_opt(&state.inspector.language);
        out.isrc = clean_opt(&state.inspector.isrc);
        out.encoder_settings = clean_opt(&state.inspector.encoder_settings);
        out.encoded_by = clean_opt(&state.inspector.encoded_by);
        out.copyright = clean_opt(&state.inspector.copyright);
    }

    Ok(out)
}

/// Trim user input; return `None` if the result is empty.
fn clean_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

// --------------------
// Helpers (state mutation)
// --------------------

fn clear_selection_and_inspector(state: &mut Sonora) {
    state.selected_track = None;
    state.selected_album = None;
    clear_inspector(state);
}

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

    // Core: show raw values (blank if None) so we don't accidentally write placeholders into tags.
    state.inspector.title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    state.inspector.artist = t.artist.clone().unwrap_or_default();
    state.inspector.album = t.album.clone().unwrap_or_default();
    state.inspector.album_artist = t.album_artist.clone().unwrap_or_default();
    state.inspector.composer = t.composer.clone().unwrap_or_default();

    state.inspector.track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.track_total = t.track_total.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.disc_no = t.disc_no.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.disc_total = t.disc_total.map(|n| n.to_string()).unwrap_or_default();

    state.inspector.year = t.year.map(|y| y.to_string()).unwrap_or_default();
    state.inspector.date = t.date.clone().unwrap_or_default();
    state.inspector.genre = t.genre.clone().unwrap_or_default();

    // Extended
    state.inspector.grouping = t.grouping.clone().unwrap_or_default();
    state.inspector.comment = t.comment.clone().unwrap_or_default();
    state.inspector.lyrics = t.lyrics.clone().unwrap_or_default();
    state.inspector.lyricist = t.lyricist.clone().unwrap_or_default();
    state.inspector.conductor = t.conductor.clone().unwrap_or_default();
    state.inspector.remixer = t.remixer.clone().unwrap_or_default();
    state.inspector.publisher = t.publisher.clone().unwrap_or_default();
    state.inspector.subtitle = t.subtitle.clone().unwrap_or_default();
    state.inspector.bpm = t.bpm.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.key = t.key.clone().unwrap_or_default();
    state.inspector.mood = t.mood.clone().unwrap_or_default();
    state.inspector.language = t.language.clone().unwrap_or_default();
    state.inspector.isrc = t.isrc.clone().unwrap_or_default();
    state.inspector.encoder_settings = t.encoder_settings.clone().unwrap_or_default();
    state.inspector.encoded_by = t.encoded_by.clone().unwrap_or_default();
    state.inspector.copyright = t.copyright.clone().unwrap_or_default();

    state.inspector_dirty = false;
}

fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
}
