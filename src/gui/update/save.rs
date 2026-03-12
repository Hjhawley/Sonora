//! gui/update/save.rs
//!
//! Turn the InspectorDraft into actual on-disk tag writes (single or batch).
//!
//! - Save targets are identified by `TrackId`, not `Vec` indices.
//! - We still update `state.tracks` (display order Vec), but we locate rows by id.
//!
//! Safety features:
//! - Mixed inspector fields are treated as "leave existing value alone".
//! - If batch saving, unchanged values that still match the primary track’s
//!   original value are also treated conservatively to reduce accidental
//!   overwrite of many files.
//!
//! Intentional behavior:
//! - We never mutate `state.tracks` until after a successful write + re-read.
//! - On write failure, UI remains consistent with disk.

use iced::Task;

use super::super::state::{InspectorField, Message, Sonora, is_mixed_display_value};
use super::super::util::{parse_optional_i32, parse_optional_u32};
use super::inspector::load_inspector_from_selection;
use super::util::spawn_blocking;
use crate::core::types::{TrackId, TrackRow};

pub(crate) fn save_inspector_to_file(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    if !state.inspector_dirty {
        state.status = "No changes to save.".to_string();
        return Task::none();
    }

    let mut ids: Vec<TrackId> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(id) = state.selected_track {
        vec![id]
    } else {
        vec![]
    };

    ids.sort_unstable();
    ids.dedup();

    if ids.is_empty() {
        state.status = "Select a track first.".to_string();
        return Task::none();
    }

    let is_batch = ids.len() > 1;
    let primary_id = state.selected_track;
    let primary_row: Option<&TrackRow> = primary_id.and_then(|id| state.track_by_id(id));

    let mut rows_to_write: Vec<(TrackId, TrackRow)> = Vec::with_capacity(ids.len());
    for &id in &ids {
        match build_row_from_inspector_for_id(state, id, is_batch, primary_row) {
            Ok(r) => rows_to_write.push((id, r)),
            Err(e) => {
                state.status = e;
                return Task::none();
            }
        }
    }

    state.saving = true;
    state.status = if ids.len() == 1 {
        "Writing tags to file...".to_string()
    } else {
        format!("Writing tags to {} files...", ids.len())
    };

    if rows_to_write.len() == 1 {
        let (id, row_to_write) = rows_to_write.remove(0);

        return Task::perform(
            spawn_blocking(move || {
                crate::core::tags::write_track_row(&row_to_write, true).and_then(|_| {
                    let (mut r, failed) =
                        crate::core::tags::read_track_row(row_to_write.path.clone());
                    if failed {
                        Err("Wrote tags, but failed to re-read them".to_string())
                    } else {
                        r.id = row_to_write.id;
                        Ok(r)
                    }
                })
            }),
            move |res| Message::SaveFinished(id, res),
        );
    }

    Task::perform(
        spawn_blocking(move || {
            let mut out: Vec<(TrackId, TrackRow)> = Vec::new();

            for (id, row) in rows_to_write {
                crate::core::tags::write_track_row(&row, true)
                    .map_err(|e| format!("Write failed for track {id}: {e}"))?;

                let (mut r, failed) = crate::core::tags::read_track_row(row.path.clone());
                if failed {
                    return Err(format!(
                        "Wrote tags for track {id}, but failed to re-read them"
                    ));
                }

                r.id = row.id;
                out.push((id, r));
            }

            Ok(out)
        }),
        Message::SaveFinishedBatch,
    )
}

pub(crate) fn save_finished(
    state: &mut Sonora,
    id: TrackId,
    result: Result<TrackRow, String>,
) -> Task<Message> {
    state.saving = false;

    match result {
        Ok(new_row) => {
            if let Some(slot) = state.track_by_id_mut(id) {
                *slot = new_row;
                state.rebuild_library_derived_state();
                load_inspector_from_selection(state);
            } else {
                state.status = "Tags written, but selection changed (rescan?).".to_string();
                state.inspector_dirty = false;
                return Task::none();
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
    result: Result<Vec<(TrackId, TrackRow)>, String>,
) -> Task<Message> {
    state.saving = false;

    match result {
        Ok(rows) => {
            for (id, row) in rows {
                if let Some(slot) = state.track_by_id_mut(id) {
                    *slot = row;
                }
            }

            state.rebuild_library_derived_state();
            load_inspector_from_selection(state);

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
    load_inspector_from_selection(state);
    Task::none()
}

fn build_row_from_inspector_for_id(
    state: &Sonora,
    id: TrackId,
    is_batch: bool,
    primary_row: Option<&TrackRow>,
) -> Result<TrackRow, String> {
    let mut out = state
        .track_by_id(id)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    let mut errs: Vec<&'static str> = Vec::new();

    let track_no = parse_u32_mixed(
        state,
        InspectorField::TrackNo,
        &state.inspector.track_no,
        out.track_no,
        "Track #",
        &mut errs,
    )?;
    let track_total = parse_u32_mixed(
        state,
        InspectorField::TrackTotal,
        &state.inspector.track_total,
        out.track_total,
        "Track total",
        &mut errs,
    )?;
    let disc_no = parse_u32_mixed(
        state,
        InspectorField::DiscNo,
        &state.inspector.disc_no,
        out.disc_no,
        "Disc #",
        &mut errs,
    )?;
    let disc_total = parse_u32_mixed(
        state,
        InspectorField::DiscTotal,
        &state.inspector.disc_total,
        out.disc_total,
        "Disc total",
        &mut errs,
    )?;
    let year = parse_i32_mixed(
        state,
        InspectorField::Year,
        &state.inspector.year,
        out.year,
        "Year",
        &mut errs,
    )?;
    let bpm = parse_u32_mixed(
        state,
        InspectorField::Bpm,
        &state.inspector.bpm,
        out.bpm,
        "BPM",
        &mut errs,
    )?;

    if !errs.is_empty() {
        return Err(format!("Not saved: invalid {}", errs.join(", ")));
    }

    let primary = primary_row;

    apply_opt_mixed_batch(
        state,
        InspectorField::Title,
        &mut out.title,
        &state.inspector.title,
        is_batch,
        primary.and_then(|p| p.title.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Artist,
        &mut out.artist,
        &state.inspector.artist,
        is_batch,
        primary.and_then(|p| p.artist.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Album,
        &mut out.album,
        &state.inspector.album,
        is_batch,
        primary.and_then(|p| p.album.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::AlbumArtist,
        &mut out.album_artist,
        &state.inspector.album_artist,
        is_batch,
        primary.and_then(|p| p.album_artist.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Composer,
        &mut out.composer,
        &state.inspector.composer,
        is_batch,
        primary.and_then(|p| p.composer.as_deref()),
    );

    out.track_no = track_no;
    out.track_total = track_total;
    out.disc_no = disc_no;
    out.disc_total = disc_total;
    out.year = year;

    apply_opt_mixed_batch(
        state,
        InspectorField::Genre,
        &mut out.genre,
        &state.inspector.genre,
        is_batch,
        primary.and_then(|p| p.genre.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Grouping,
        &mut out.grouping,
        &state.inspector.grouping,
        is_batch,
        primary.and_then(|p| p.grouping.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Comment,
        &mut out.comment,
        &state.inspector.comment,
        is_batch,
        primary.and_then(|p| p.comment.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Lyrics,
        &mut out.lyrics,
        &state.inspector.lyrics,
        is_batch,
        primary.and_then(|p| p.lyrics.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Lyricist,
        &mut out.lyricist,
        &state.inspector.lyricist,
        is_batch,
        primary.and_then(|p| p.lyricist.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Date,
        &mut out.date,
        &state.inspector.date,
        is_batch,
        primary.and_then(|p| p.date.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Conductor,
        &mut out.conductor,
        &state.inspector.conductor,
        is_batch,
        primary.and_then(|p| p.conductor.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Remixer,
        &mut out.remixer,
        &state.inspector.remixer,
        is_batch,
        primary.and_then(|p| p.remixer.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Publisher,
        &mut out.publisher,
        &state.inspector.publisher,
        is_batch,
        primary.and_then(|p| p.publisher.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Subtitle,
        &mut out.subtitle,
        &state.inspector.subtitle,
        is_batch,
        primary.and_then(|p| p.subtitle.as_deref()),
    );

    out.bpm = bpm;

    apply_opt_mixed_batch(
        state,
        InspectorField::Key,
        &mut out.key,
        &state.inspector.key,
        is_batch,
        primary.and_then(|p| p.key.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Mood,
        &mut out.mood,
        &state.inspector.mood,
        is_batch,
        primary.and_then(|p| p.mood.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Language,
        &mut out.language,
        &state.inspector.language,
        is_batch,
        primary.and_then(|p| p.language.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Isrc,
        &mut out.isrc,
        &state.inspector.isrc,
        is_batch,
        primary.and_then(|p| p.isrc.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::EncoderSettings,
        &mut out.encoder_settings,
        &state.inspector.encoder_settings,
        is_batch,
        primary.and_then(|p| p.encoder_settings.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::EncodedBy,
        &mut out.encoded_by,
        &state.inspector.encoded_by,
        is_batch,
        primary.and_then(|p| p.encoded_by.as_deref()),
    );
    apply_opt_mixed_batch(
        state,
        InspectorField::Copyright,
        &mut out.copyright,
        &state.inspector.copyright,
        is_batch,
        primary.and_then(|p| p.copyright.as_deref()),
    );

    Ok(out)
}

fn apply_opt_mixed_batch(
    state: &Sonora,
    field: InspectorField,
    dst: &mut Option<String>,
    input: &str,
    is_batch: bool,
    primary_value: Option<&str>,
) {
    if state.inspector_mixed.get(&field).copied().unwrap_or(false) {
        return;
    }

    let t = input.trim();
    if is_mixed_display_value(t) {
        return;
    }

    if is_batch {
        if let Some(pv) = primary_value {
            if t == pv.trim() {
                return;
            }
        }
    }

    if t.is_empty() {
        *dst = None;
    } else {
        *dst = Some(t.to_string());
    }
}

fn parse_u32_mixed(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    current: Option<u32>,
    label: &'static str,
    errs: &mut Vec<&'static str>,
) -> Result<Option<u32>, String> {
    if state.inspector_mixed.get(&field).copied().unwrap_or(false) {
        return Ok(current);
    }

    let t = input.trim();
    if is_mixed_display_value(t) {
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

fn parse_i32_mixed(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    current: Option<i32>,
    label: &'static str,
    errs: &mut Vec<&'static str>,
) -> Result<Option<i32>, String> {
    if state.inspector_mixed.get(&field).copied().unwrap_or(false) {
        return Ok(current);
    }

    let t = input.trim();
    if is_mixed_display_value(t) {
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
