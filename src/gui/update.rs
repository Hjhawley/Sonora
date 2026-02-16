//! Update logic.
//! Mutates state in response to Messages.

use iced::Task;
use iced::futures::channel::oneshot;
use std::path::{Path, PathBuf};

use crate::core;
use crate::core::types::TrackRow;

use super::state::{Message, Sonora, TEST_ROOT, ViewMode};
use super::util::{clean_optional_string, filename_stem, parse_optional_i32, parse_optional_u32};

pub(crate) fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        // Roots
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
            state.status = "Scanning...".to_string();

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

        // Album selection (toggle collapse)
        Message::SelectAlbum(key) => {
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

        // Inspector toggles
        Message::ToggleExtended(v) => {
            state.show_extended = v;
            Task::none()
        }

        // Inspector typing (core)
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
        Message::EditAlbumArtist(s) => {
            state.inspector.album_artist = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditComposer(s) => {
            state.inspector.composer = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditTrackNo(s) => {
            state.inspector.track_no = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditTrackTotal(s) => {
            state.inspector.track_total = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditDiscNo(s) => {
            state.inspector.disc_no = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditDiscTotal(s) => {
            state.inspector.disc_total = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditYear(s) => {
            state.inspector.year = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditDate(s) => {
            state.inspector.date = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditGenre(s) => {
            state.inspector.genre = s;
            state.inspector_dirty = true;
            Task::none()
        }

        // Inspector typing (extended)
        Message::EditLyricist(s) => {
            state.inspector.lyricist = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditConductor(s) => {
            state.inspector.conductor = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditRemixer(s) => {
            state.inspector.remixer = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditPublisher(s) => {
            state.inspector.publisher = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditGrouping(s) => {
            state.inspector.grouping = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditSubtitle(s) => {
            state.inspector.subtitle = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditBpm(s) => {
            state.inspector.bpm = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditKey(s) => {
            state.inspector.key = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditMood(s) => {
            state.inspector.mood = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditLanguage(s) => {
            state.inspector.language = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditIsrc(s) => {
            state.inspector.isrc = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditEncoderSettings(s) => {
            state.inspector.encoder_settings = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditEncodedBy(s) => {
            state.inspector.encoded_by = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditCopyright(s) => {
            state.inspector.copyright = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditComment(s) => {
            state.inspector.comment = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditLyrics(s) => {
            state.inspector.lyrics = s;
            state.inspector_dirty = true;
            Task::none()
        }

        // Save to disk
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
                return Task::none();
            }

            // Build the row we actually want to write (from inspector), with validation.
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
                async move {
                    let (tx, rx) = oneshot::channel::<Result<TrackRow, String>>();

                    std::thread::spawn(move || {
                        let res = crate::core::tags::write_track_row(&row_to_write, write_extended)
                            .and_then(|_| {
                                let (r, failed) =
                                    crate::core::tags::read_track_row(row_to_write.path.clone());
                                if failed {
                                    Err("Wrote tags, but failed to re-read them".to_string())
                                } else {
                                    Ok(r)
                                }
                            });

                        let _ = tx.send(res);
                    });

                    rx.await
                        .map_err(|_| "Save thread dropped without returning".to_string())?
                },
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

/// Build the TrackRow to write by applying the inspector draft onto the currently selected row.
/// Mirrors your “save-to-memory” semantics:
/// - core fields always applied
/// - extended fields applied only if `state.show_extended == true`
/// - empty/whitespace => None (which your writer treats as “remove tag”)
fn build_row_from_inspector(state: &Sonora, i: usize) -> Result<TrackRow, String> {
    let base = state
        .tracks
        .get(i)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    // Parse numeric fields (collect errors)
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
        base.bpm
    };

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    let mut out = base;

    // Core
    out.title = clean_optional_string(&state.inspector.title);
    out.artist = clean_optional_string(&state.inspector.artist);
    out.album = clean_optional_string(&state.inspector.album);
    out.album_artist = clean_optional_string(&state.inspector.album_artist);
    out.composer = clean_optional_string(&state.inspector.composer);

    out.track_no = track_no;
    out.track_total = track_total;
    out.disc_no = disc_no;
    out.disc_total = disc_total;

    out.year = year;
    out.date = clean_optional_string(&state.inspector.date);
    out.genre = clean_optional_string(&state.inspector.genre);

    // Extended (only if user is looking at them — otherwise keep existing)
    if state.show_extended {
        out.lyricist = clean_optional_string(&state.inspector.lyricist);
        out.conductor = clean_optional_string(&state.inspector.conductor);
        out.remixer = clean_optional_string(&state.inspector.remixer);
        out.publisher = clean_optional_string(&state.inspector.publisher);
        out.grouping = clean_optional_string(&state.inspector.grouping);
        out.subtitle = clean_optional_string(&state.inspector.subtitle);
        out.bpm = bpm;
        out.key = clean_optional_string(&state.inspector.key);
        out.mood = clean_optional_string(&state.inspector.mood);
        out.language = clean_optional_string(&state.inspector.language);
        out.isrc = clean_optional_string(&state.inspector.isrc);
        out.encoder_settings = clean_optional_string(&state.inspector.encoder_settings);
        out.encoded_by = clean_optional_string(&state.inspector.encoded_by);
        out.copyright = clean_optional_string(&state.inspector.copyright);
        out.comment = clean_optional_string(&state.inspector.comment);
        out.lyrics = clean_optional_string(&state.inspector.lyrics);
    }

    Ok(out)
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

    // Core: show actual values (blank if None) so we don't write "Unknown" into tags later.
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
    state.inspector.lyricist = t.lyricist.clone().unwrap_or_default();
    state.inspector.conductor = t.conductor.clone().unwrap_or_default();
    state.inspector.remixer = t.remixer.clone().unwrap_or_default();
    state.inspector.publisher = t.publisher.clone().unwrap_or_default();
    state.inspector.grouping = t.grouping.clone().unwrap_or_default();
    state.inspector.subtitle = t.subtitle.clone().unwrap_or_default();
    state.inspector.bpm = t.bpm.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.key = t.key.clone().unwrap_or_default();
    state.inspector.mood = t.mood.clone().unwrap_or_default();
    state.inspector.language = t.language.clone().unwrap_or_default();
    state.inspector.isrc = t.isrc.clone().unwrap_or_default();
    state.inspector.encoder_settings = t.encoder_settings.clone().unwrap_or_default();
    state.inspector.encoded_by = t.encoded_by.clone().unwrap_or_default();
    state.inspector.copyright = t.copyright.clone().unwrap_or_default();
    state.inspector.comment = t.comment.clone().unwrap_or_default();
    state.inspector.lyrics = t.lyrics.clone().unwrap_or_default();

    state.inspector_dirty = false;
}

fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
}
