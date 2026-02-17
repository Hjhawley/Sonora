use iced::Task;

use super::super::state::{KEEP_SENTINEL, Message, Sonora};
use super::super::util::{parse_optional_i32, parse_optional_u32};
use super::helpers::spawn_blocking;
use super::inspector::load_inspector_from_track;

pub(crate) fn save_inspector_to_file(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.inspector_dirty {
        state.status = "No changes to save.".to_string();
        return Task::none();
    }

    // Determine which indices we are saving to.
    // - If selected_tracks has anything, use it
    // - Else fall back to selected_track
    let mut indices: Vec<usize> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(i) = state.selected_track {
        vec![i]
    } else {
        vec![]
    };

    indices.sort_unstable();
    indices.dedup();

    if indices.is_empty() {
        state.status = "Select a track first.".to_string();
        return Task::none();
    }

    // Validate + build rows to write
    let mut rows_to_write = Vec::with_capacity(indices.len());
    for &i in &indices {
        if i >= state.tracks.len() {
            state.status = "Invalid selection (rescan?).".to_string();
            return Task::none();
        }
        match build_row_from_inspector_for_index(state, i) {
            Ok(r) => rows_to_write.push((i, r)),
            Err(e) => {
                state.status = e;
                return Task::none();
            }
        }
    }

    state.saving = true;
    state.status = if indices.len() == 1 {
        "Writing tags to file...".to_string()
    } else {
        format!("Writing tags to {} files...", indices.len())
    };

    let write_extended = state.show_extended;

    // Single-file path: keep your old message shape (SaveFinished)
    if rows_to_write.len() == 1 {
        let (i, row_to_write) = rows_to_write.remove(0);

        return Task::perform(
            spawn_blocking(move || {
                crate::core::tags::write_track_row(&row_to_write, write_extended).and_then(|_| {
                    let (r, failed) = crate::core::tags::read_track_row(row_to_write.path.clone());
                    if failed {
                        Err("Wrote tags, but failed to re-read them".to_string())
                    } else {
                        Ok(r)
                    }
                })
            }),
            move |res| Message::SaveFinished(i, res),
        );
    }

    // Batch path: write all, re-read all, return Vec<(idx, TrackRow)>
    Task::perform(
        spawn_blocking(move || {
            let mut out: Vec<(usize, crate::core::types::TrackRow)> = Vec::new();

            for (i, row) in rows_to_write {
                crate::core::tags::write_track_row(&row, write_extended)
                    .map_err(|e| format!("Write failed for index {i}: {e}"))?;

                let (r, failed) = crate::core::tags::read_track_row(row.path.clone());
                if failed {
                    return Err(format!(
                        "Wrote tags for index {i}, but failed to re-read them"
                    ));
                }
                out.push((i, r));
            }

            Ok(out)
        }),
        Message::SaveFinishedBatch,
    )
}

pub(crate) fn save_finished(
    state: &mut Sonora,
    i: usize,
    result: Result<crate::core::types::TrackRow, String>,
) -> Task<Message> {
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

pub(crate) fn save_finished_batch(
    state: &mut Sonora,
    result: Result<Vec<(usize, crate::core::types::TrackRow)>, String>,
) -> Task<Message> {
    state.saving = false;

    match result {
        Ok(rows) => {
            for (i, row) in rows {
                if i < state.tracks.len() {
                    state.tracks[i] = row;
                }
            }

            // Reload inspector from primary selection (whatever it currently is)
            load_inspector_from_track(state);

            state.inspector_dirty = false;
            state.status = "Batch tags written to files.".to_string();
        }
        Err(e) => {
            state.status = format!("Batch save failed: {e}");
        }
    }

    Task::none()
}

pub(crate) fn revert_inspector(state: &mut Sonora) -> Task<Message> {
    load_inspector_from_track(state);
    Task::none()
}

// -------------------------
// Batch-aware row builder
// -------------------------

fn build_row_from_inspector_for_index(
    state: &Sonora,
    i: usize,
) -> Result<crate::core::types::TrackRow, String> {
    let mut out = state
        .tracks
        .get(i)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    // Parse numeric fields; BUT treat "<keep>" as "do not change this number"
    let mut errs: Vec<&'static str> = Vec::new();

    let track_no = parse_u32_keep(
        &state.inspector.track_no,
        out.track_no,
        "Track #",
        &mut errs,
    )?;
    let track_total = parse_u32_keep(
        &state.inspector.track_total,
        out.track_total,
        "Track total",
        &mut errs,
    )?;
    let disc_no = parse_u32_keep(&state.inspector.disc_no, out.disc_no, "Disc #", &mut errs)?;
    let disc_total = parse_u32_keep(
        &state.inspector.disc_total,
        out.disc_total,
        "Disc total",
        &mut errs,
    )?;

    let year = parse_i32_keep(&state.inspector.year, out.year, "Year", &mut errs)?;

    // BPM is extended-only. If extended not shown, preserve existing.
    let bpm = if state.show_extended {
        parse_u32_keep(&state.inspector.bpm, out.bpm, "BPM", &mut errs)?
    } else {
        out.bpm
    };

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    // -------------------------
    // Standard (always applied)
    // KEEP_SENTINEL means "leave this field as-is"
    // -------------------------
    apply_opt_keep(&mut out.title, &state.inspector.title);
    apply_opt_keep(&mut out.artist, &state.inspector.artist);
    apply_opt_keep(&mut out.album, &state.inspector.album);
    apply_opt_keep(&mut out.album_artist, &state.inspector.album_artist);
    apply_opt_keep(&mut out.composer, &state.inspector.composer);

    out.track_no = track_no;
    out.track_total = track_total;
    out.disc_no = disc_no;
    out.disc_total = disc_total;

    out.year = year;
    apply_opt_keep(&mut out.genre, &state.inspector.genre);

    apply_opt_keep(&mut out.grouping, &state.inspector.grouping);
    apply_opt_keep(&mut out.comment, &state.inspector.comment);
    apply_opt_keep(&mut out.lyrics, &state.inspector.lyrics);
    apply_opt_keep(&mut out.lyricist, &state.inspector.lyricist);

    // -------------------------
    // Extended (only if visible)
    // -------------------------
    if state.show_extended {
        apply_opt_keep(&mut out.date, &state.inspector.date);

        apply_opt_keep(&mut out.conductor, &state.inspector.conductor);
        apply_opt_keep(&mut out.remixer, &state.inspector.remixer);
        apply_opt_keep(&mut out.publisher, &state.inspector.publisher);
        apply_opt_keep(&mut out.subtitle, &state.inspector.subtitle);

        out.bpm = bpm;
        apply_opt_keep(&mut out.key, &state.inspector.key);
        apply_opt_keep(&mut out.mood, &state.inspector.mood);
        apply_opt_keep(&mut out.language, &state.inspector.language);
        apply_opt_keep(&mut out.isrc, &state.inspector.isrc);
        apply_opt_keep(&mut out.encoder_settings, &state.inspector.encoder_settings);
        apply_opt_keep(&mut out.encoded_by, &state.inspector.encoded_by);
        apply_opt_keep(&mut out.copyright, &state.inspector.copyright);
    }

    Ok(out)
}

/// Applies a text input to an Option<String> field.
/// - If input is "<keep>" => do nothing
/// - Else if trimmed empty => set None (delete tag)
/// - Else => set Some(trimmed)
fn apply_opt_keep(dst: &mut Option<String>, input: &str) {
    let t = input.trim();

    if t == KEEP_SENTINEL {
        return;
    }
    if t.is_empty() {
        *dst = None;
    } else {
        *dst = Some(t.to_string());
    }
}

fn parse_u32_keep(
    input: &str,
    current: Option<u32>,
    label: &'static str,
    errs: &mut Vec<&'static str>,
) -> Result<Option<u32>, String> {
    let t = input.trim();
    if t == KEEP_SENTINEL {
        return Ok(current);
    }
    if t.is_empty() {
        return Ok(None);
    }

    let v = parse_optional_u32(t)
        .inspect_err(|_| errs.push(label))
        .ok()
        .flatten();

    Ok(v)
}

fn parse_i32_keep(
    input: &str,
    current: Option<i32>,
    label: &'static str,
    errs: &mut Vec<&'static str>,
) -> Result<Option<i32>, String> {
    let t = input.trim();
    if t == KEEP_SENTINEL {
        return Ok(current);
    }
    if t.is_empty() {
        return Ok(None);
    }

    let v = parse_optional_i32(t)
        .inspect_err(|_| errs.push(label))
        .ok()
        .flatten();

    Ok(v)
}
