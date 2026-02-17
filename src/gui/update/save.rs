use iced::Task;

use super::super::state::{Message, Sonora};
use super::super::util::{parse_optional_i32, parse_optional_u32};
use super::helpers::spawn_blocking;
use super::inspector::load_inspector_from_track;

const MIXED_SENTINEL: &str = "< mixed metadata >";

pub(crate) fn save_inspector_to_file(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.inspector_dirty {
        state.status = "No changes to save.".to_string();
        return Task::none();
    }

    // Batch selection support:
    // - if selected_tracks non-empty => write to all of them
    // - else fall back to selected_track
    let targets: Vec<usize> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(i) = state.selected_track {
        vec![i]
    } else {
        vec![]
    };

    if targets.is_empty() {
        state.status = "Select one or more tracks first.".to_string();
        return Task::none();
    }
    if targets.iter().any(|&i| i >= state.tracks.len()) {
        state.status = "Invalid selection (rescan?).".to_string();
        return Task::none();
    }

    // Build N rows to write (one per target), with validation + "< mixed metadata >" semantics.
    let rows_to_write = match build_rows_from_inspector(state, &targets) {
        Ok(v) => v,
        Err(e) => {
            state.status = e;
            return Task::none();
        }
    };

    state.saving = true;
    state.status = if rows_to_write.len() == 1 {
        "Writing tags to file...".to_string()
    } else {
        format!("Writing tags to {} files...", rows_to_write.len())
    };

    let write_extended = state.show_extended;

    Task::perform(
        spawn_blocking(move || {
            // write all; then re-read all
            let mut updated: Vec<(usize, crate::core::types::TrackRow)> = Vec::new();

            for (idx, row) in rows_to_write {
                crate::core::tags::write_track_row(&row, write_extended)?;

                let (r, failed) = crate::core::tags::read_track_row(row.path.clone());
                if failed {
                    return Err("Wrote tags, but failed to re-read them".to_string());
                }
                updated.push((idx, r));
            }

            Ok(updated)
        }),
        Message::SaveFinishedBatch,
    )
}

pub(crate) fn save_finished(
    state: &mut Sonora,
    i: usize,
    result: Result<crate::core::types::TrackRow, String>,
) -> Task<Message> {
    // legacy single-file message still supported
    state.saving = false;

    match result {
        Ok(new_row) => {
            if i < state.tracks.len() {
                state.tracks[i] = new_row;
            }
            state.inspector_dirty = false;
            load_inspector_from_track(state);
            state.status = "Tags written to file.".to_string();
        }
        Err(e) => {
            state.status = format!("Save failed: {e}");
        }
    }

    Task::none()
}

pub(crate) fn save_finished_batch(
    state: &mut Sonora,
    result: Result<Vec<(usize, crate::core::types::TrackRow)>, String>,
) -> Task<Message> {
    state.saving = false;

    match result {
        Ok(updated) => {
            for (i, row) in updated {
                if i < state.tracks.len() {
                    state.tracks[i] = row;
                }
            }
            state.inspector_dirty = false;
            load_inspector_from_track(state);
            state.status = "Tags written to files.".to_string();
        }
        Err(e) => {
            state.status = format!("Save failed: {e}");
        }
    }

    Task::none()
}

pub(crate) fn revert_inspector(state: &mut Sonora) -> Task<Message> {
    load_inspector_from_track(state);
    Task::none()
}

/// Build the TrackRows to write by applying the inspector draft onto each selected row.
///
/// Semantics:
/// - Standard fields are always eligible to apply.
/// - Extended fields are only eligible if `state.show_extended == true`.
/// - For batch editing:
///     - If an inspector field is "< mixed metadata >", we DO NOT change that field on any target.
///     - Otherwise, we apply the value to every target (including clearing it if blank).
/// - Numeric fields:
///     - "< mixed metadata >" => leave unchanged
///     - blank => None (clears)
///     - invalid => error
fn build_rows_from_inspector(
    state: &Sonora,
    targets: &[usize],
) -> Result<Vec<(usize, crate::core::types::TrackRow)>, String> {
    // Parse numeric inputs once (or decide they are keep/blank).
    let mut errs: Vec<&'static str> = Vec::new();

    let track_no = parse_num_u32(&state.inspector.track_no, "Track #", &mut errs, state)?;
    let track_total = parse_num_u32(
        &state.inspector.track_total,
        "Track total",
        &mut errs,
        state,
    )?;
    let disc_no = parse_num_u32(&state.inspector.disc_no, "Disc #", &mut errs, state)?;
    let disc_total = parse_num_u32(&state.inspector.disc_total, "Disc total", &mut errs, state)?;
    let year = parse_num_i32(&state.inspector.year, "Year", &mut errs, state)?;
    let bpm = if state.show_extended {
        parse_num_u32(&state.inspector.bpm, "BPM", &mut errs, state)?
    } else {
        NumEditU32::Keep
    };

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    let mut out_rows: Vec<(usize, crate::core::types::TrackRow)> =
        Vec::with_capacity(targets.len());

    for &i in targets {
        let mut out = state
            .tracks
            .get(i)
            .cloned()
            .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

        // -------------------------
        // Standard (always eligible)
        // -------------------------
        apply_opt_string(&mut out.title, &state.inspector.title);
        apply_opt_string(&mut out.artist, &state.inspector.artist);
        apply_opt_string(&mut out.album, &state.inspector.album);
        apply_opt_string(&mut out.album_artist, &state.inspector.album_artist);
        apply_opt_string(&mut out.composer, &state.inspector.composer);

        apply_num_u32(&mut out.track_no, track_no);
        apply_num_u32(&mut out.track_total, track_total);
        apply_num_u32(&mut out.disc_no, disc_no);
        apply_num_u32(&mut out.disc_total, disc_total);
        apply_num_i32(&mut out.year, year);

        apply_opt_string(&mut out.genre, &state.inspector.genre);

        apply_opt_string(&mut out.grouping, &state.inspector.grouping);
        apply_opt_string(&mut out.comment, &state.inspector.comment);
        apply_opt_string(&mut out.lyrics, &state.inspector.lyrics);
        apply_opt_string(&mut out.lyricist, &state.inspector.lyricist);

        // -------------------------
        // Extended (only if visible)
        // -------------------------
        if state.show_extended {
            apply_opt_string(&mut out.date, &state.inspector.date);

            apply_opt_string(&mut out.conductor, &state.inspector.conductor);
            apply_opt_string(&mut out.remixer, &state.inspector.remixer);
            apply_opt_string(&mut out.publisher, &state.inspector.publisher);
            apply_opt_string(&mut out.subtitle, &state.inspector.subtitle);

            apply_num_u32(&mut out.bpm, bpm);

            apply_opt_string(&mut out.key, &state.inspector.key);
            apply_opt_string(&mut out.mood, &state.inspector.mood);
            apply_opt_string(&mut out.language, &state.inspector.language);
            apply_opt_string(&mut out.isrc, &state.inspector.isrc);
            apply_opt_string(&mut out.encoder_settings, &state.inspector.encoder_settings);
            apply_opt_string(&mut out.encoded_by, &state.inspector.encoded_by);
            apply_opt_string(&mut out.copyright, &state.inspector.copyright);
        }

        out_rows.push((i, out));
    }

    Ok(out_rows)
}

// --------------------
// "< mixed metadata >" helpers
// --------------------

fn is_keep(s: &str) -> bool {
    s.trim() == MIXED_SENTINEL
}

fn clean_opt(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Apply a string input to an Option<String> field:
/// - "< mixed metadata >" => do nothing
/// - otherwise => set to trimmed string or None if blank
fn apply_opt_string(dst: &mut Option<String>, input: &str) {
    if is_keep(input) {
        return;
    }
    *dst = clean_opt(input);
}

#[derive(Clone, Copy)]
enum NumEditU32 {
    Keep,
    Set(Option<u32>),
}

#[derive(Clone, Copy)]
enum NumEditI32 {
    Keep,
    Set(Option<i32>),
}

fn apply_num_u32(dst: &mut Option<u32>, edit: NumEditU32) {
    match edit {
        NumEditU32::Keep => {}
        NumEditU32::Set(v) => *dst = v,
    }
}

fn apply_num_i32(dst: &mut Option<i32>, edit: NumEditI32) {
    match edit {
        NumEditI32::Keep => {}
        NumEditI32::Set(v) => *dst = v,
    }
}

fn parse_num_u32(
    input: &str,
    label: &'static str,
    errs: &mut Vec<&'static str>,
    _state: &Sonora,
) -> Result<NumEditU32, String> {
    if is_keep(input) {
        return Ok(NumEditU32::Keep);
    }
    match parse_optional_u32(input) {
        Ok(v) => Ok(NumEditU32::Set(v)),
        Err(_) => {
            errs.push(label);
            Ok(NumEditU32::Keep)
        }
    }
}

fn parse_num_i32(
    input: &str,
    label: &'static str,
    errs: &mut Vec<&'static str>,
    _state: &Sonora,
) -> Result<NumEditI32, String> {
    if is_keep(input) {
        return Ok(NumEditI32::Keep);
    }
    match parse_optional_i32(input) {
        Ok(v) => Ok(NumEditI32::Set(v)),
        Err(_) => {
            errs.push(label);
            Ok(NumEditI32::Keep)
        }
    }
}
