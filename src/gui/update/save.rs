//! gui/update/save.rs
//!
//! Write InspectorDraft changes to disk.
//! - Save targets are identified by 'TrackId'.
//! - We still update 'state.tracks' (display order Vec), but we locate rows by id.
//! - Mixed inspector fields are treated as "leave existing value alone".
//! - If batch saving, unchanged values that still match the primary track’s
//!   original value are also treated conservatively to reduce accidental
//!   overwrite of many files.
//! - We never mutate 'state.tracks' until after a successful write + re-read.
//! - On write failure, UI remains consistent with disk.

use iced::Task;

use super::super::state::{ArtworkEdit, InspectorField, Message, Sonora, is_mixed_display_value};
use super::super::util::{
    extract_year_from_release_date, parse_optional_release_date, parse_optional_u32,
};
use super::art::reset_inspector_artwork_state;
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

    let ids = selected_ids_for_save(state);

    if ids.is_empty() {
        state.status = "Select a track first.".to_string();
        return Task::none();
    }

    let is_batch = ids.len() > 1;

    let primary_row: Option<&TrackRow> = state.selected_track.and_then(|id| state.track_by_id(id));

    let artwork_edit = state.inspector_art_edit.clone();

    let mut rows_to_write: Vec<(TrackId, TrackRow)> = Vec::with_capacity(ids.len());

    for &id in &ids {
        match build_row_from_inspector_for_id(state, id, is_batch, primary_row) {
            Ok(row) => rows_to_write.push((id, row)),
            Err(error) => {
                state.status = error;
                return Task::none();
            }
        }
    }

    state.saving = true;

    if ids.len() == 1 {
        state.status = "Writing tags to file...".to_string();
    } else {
        state.status = format!("Writing tags to {} files...", ids.len());
    }

    if rows_to_write.len() == 1 {
        let (id, row_to_write) = rows_to_write.remove(0);
        let art_edit = artwork_edit.clone();

        return Task::perform(
            spawn_blocking(move || write_and_reread_row(row_to_write, art_edit)),
            move |result| Message::SaveFinished(id, result),
        );
    }

    Task::perform(
        spawn_blocking(move || {
            let mut output: Vec<(TrackId, TrackRow)> = Vec::new();

            for (id, row) in rows_to_write {
                let reread = write_and_reread_row(row, artwork_edit.clone())
                    .map_err(|error| format!("Save failed for track {id}: {error}"))?;

                output.push((id, reread));
            }

            Ok(output)
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
            } else {
                state.status = "Tags written, but selection changed (rescan?).".to_string();

                state.inspector_dirty = false;
                reset_inspector_artwork_state(state);
                return Task::none();
            }

            apply_saved_artwork_to_cache(state, &[id]);

            state.rebuild_library_caches();
            load_inspector_from_selection(state);

            state.inspector_dirty = false;
            reset_inspector_artwork_state(state);
            state.status = "Tags written to file.".to_string();
        }

        Err(error) => {
            state.status = format!("Save failed: {error}");
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
            let mut ids: Vec<TrackId> = Vec::with_capacity(rows.len());

            for (id, row) in rows {
                ids.push(id);

                if let Some(slot) = state.track_by_id_mut(id) {
                    *slot = row;
                }
            }

            apply_saved_artwork_to_cache(state, &ids);

            state.rebuild_library_caches();
            load_inspector_from_selection(state);

            state.inspector_dirty = false;
            reset_inspector_artwork_state(state);
            state.status = "Batch tags written to files.".to_string();
        }

        Err(error) => {
            state.status = format!("Batch save failed: {error}");
        }
    }

    Task::none()
}

fn selected_ids_for_save(state: &Sonora) -> Vec<TrackId> {
    let mut ids: Vec<TrackId> = Vec::new();

    if !state.selected_tracks.is_empty() {
        ids.extend(state.selected_tracks.iter().copied());
    } else if let Some(id) = state.selected_track {
        ids.push(id);
    }

    ids.sort_unstable();
    ids.dedup();

    ids
}

fn write_and_reread_row(
    row_to_write: TrackRow,
    artwork_edit: ArtworkEdit,
) -> Result<TrackRow, String> {
    crate::core::tags::write_track_row(&row_to_write, true)?;

    match artwork_edit {
        ArtworkEdit::Unchanged => {}

        ArtworkEdit::Remove => {
            let _ = crate::core::tags::remove_embedded_art(&row_to_write.path)?;
        }

        ArtworkEdit::Replace { bytes, mime, .. } => {
            crate::core::tags::set_embedded_art(&row_to_write.path, &bytes, &mime)?;
        }
    }

    let (mut reread, failed) = crate::core::tags::read_track_row(row_to_write.path.clone());

    if failed {
        return Err("Wrote tags, but failed to re-read them".to_string());
    }

    reread.id = row_to_write.id;

    Ok(reread)
}

fn apply_saved_artwork_to_cache(state: &mut Sonora, ids: &[TrackId]) {
    match &state.inspector_art_edit {
        ArtworkEdit::Unchanged => {}

        ArtworkEdit::Remove => {
            for id in ids {
                state.cover_cache.remove(id);
            }
        }

        ArtworkEdit::Replace { preview, .. } => {
            for id in ids {
                state.cover_cache.insert(*id, preview.clone());
            }
        }
    }
}

fn build_row_from_inspector_for_id(
    state: &Sonora,
    id: TrackId,
    is_batch: bool,
    primary_row: Option<&TrackRow>,
) -> Result<TrackRow, String> {
    let mut output = state
        .track_by_id(id)
        .cloned()
        .ok_or_else(|| "Invalid selection (rescan?).".to_string())?;

    let mut errors: Vec<&'static str> = Vec::new();

    output.track_no = parse_u32_mixed(
        state,
        InspectorField::TrackNo,
        &state.inspector.track_no,
        output.track_no,
        "Track #",
        &mut errors,
    )?;

    output.track_total = parse_u32_mixed(
        state,
        InspectorField::TrackTotal,
        &state.inspector.track_total,
        output.track_total,
        "Track total",
        &mut errors,
    )?;

    output.disc_no = parse_u32_mixed(
        state,
        InspectorField::DiscNo,
        &state.inspector.disc_no,
        output.disc_no,
        "Disc #",
        &mut errors,
    )?;

    output.disc_total = parse_u32_mixed(
        state,
        InspectorField::DiscTotal,
        &state.inspector.disc_total,
        output.disc_total,
        "Disc total",
        &mut errors,
    )?;

    output.bpm = parse_u32_mixed(
        state,
        InspectorField::Bpm,
        &state.inspector.bpm,
        output.bpm,
        "BPM",
        &mut errors,
    )?;

    output.release_date = parse_release_date_mixed(
        state,
        &state.inspector.release_date,
        output.release_date.clone(),
        &mut errors,
    )?;

    output.year = extract_year_from_release_date(output.release_date.as_deref());

    if !errors.is_empty() {
        return Err(format!("Not saved: invalid {}", errors.join(", ")));
    }

    apply_basic_text_fields(state, &mut output, is_batch, primary_row);

    apply_extended_text_fields(state, &mut output, is_batch, primary_row);

    Ok(output)
}

fn apply_basic_text_fields(
    state: &Sonora,
    output: &mut TrackRow,
    is_batch: bool,
    primary_row: Option<&TrackRow>,
) {
    apply_opt_mixed_batch(
        state,
        InspectorField::Title,
        &mut output.title,
        &state.inspector.title,
        is_batch,
        primary_row.and_then(|row| row.title.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Artist,
        &mut output.artist,
        &state.inspector.artist,
        is_batch,
        primary_row.and_then(|row| row.artist.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Album,
        &mut output.album,
        &state.inspector.album,
        is_batch,
        primary_row.and_then(|row| row.album.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::AlbumArtist,
        &mut output.album_artist,
        &state.inspector.album_artist,
        is_batch,
        primary_row.and_then(|row| row.album_artist.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Composer,
        &mut output.composer,
        &state.inspector.composer,
        is_batch,
        primary_row.and_then(|row| row.composer.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Genre,
        &mut output.genre,
        &state.inspector.genre,
        is_batch,
        primary_row.and_then(|row| row.genre.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Grouping,
        &mut output.grouping,
        &state.inspector.grouping,
        is_batch,
        primary_row.and_then(|row| row.grouping.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::ContentGroup,
        &mut output.content_group,
        &state.inspector.content_group,
        is_batch,
        primary_row.and_then(|row| row.content_group.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Comment,
        &mut output.comment,
        &state.inspector.comment,
        is_batch,
        primary_row.and_then(|row| row.comment.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Lyrics,
        &mut output.lyrics,
        &state.inspector.lyrics,
        is_batch,
        primary_row.and_then(|row| row.lyrics.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Lyricist,
        &mut output.lyricist,
        &state.inspector.lyricist,
        is_batch,
        primary_row.and_then(|row| row.lyricist.as_deref()),
    );
}

fn apply_extended_text_fields(
    state: &Sonora,
    output: &mut TrackRow,
    is_batch: bool,
    primary_row: Option<&TrackRow>,
) {
    apply_opt_mixed_batch(
        state,
        InspectorField::Conductor,
        &mut output.conductor,
        &state.inspector.conductor,
        is_batch,
        primary_row.and_then(|row| row.conductor.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Remixer,
        &mut output.remixer,
        &state.inspector.remixer,
        is_batch,
        primary_row.and_then(|row| row.remixer.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Publisher,
        &mut output.publisher,
        &state.inspector.publisher,
        is_batch,
        primary_row.and_then(|row| row.publisher.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::EncoderSettings,
        &mut output.encoder_settings,
        &state.inspector.encoder_settings,
        is_batch,
        primary_row.and_then(|row| row.encoder_settings.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::EncodedBy,
        &mut output.encoded_by,
        &state.inspector.encoded_by,
        is_batch,
        primary_row.and_then(|row| row.encoded_by.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Subtitle,
        &mut output.subtitle,
        &state.inspector.subtitle,
        is_batch,
        primary_row.and_then(|row| row.subtitle.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Key,
        &mut output.key,
        &state.inspector.key,
        is_batch,
        primary_row.and_then(|row| row.key.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Mood,
        &mut output.mood,
        &state.inspector.mood,
        is_batch,
        primary_row.and_then(|row| row.mood.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Language,
        &mut output.language,
        &state.inspector.language,
        is_batch,
        primary_row.and_then(|row| row.language.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Isrc,
        &mut output.isrc,
        &state.inspector.isrc,
        is_batch,
        primary_row.and_then(|row| row.isrc.as_deref()),
    );

    apply_opt_mixed_batch(
        state,
        InspectorField::Copyright,
        &mut output.copyright,
        &state.inspector.copyright,
        is_batch,
        primary_row.and_then(|row| row.copyright.as_deref()),
    );
}

fn apply_opt_mixed_batch(
    state: &Sonora,
    field: InspectorField,
    destination: &mut Option<String>,
    input: &str,
    is_batch: bool,
    primary_value: Option<&str>,
) {
    if state.inspector_mixed.get(&field).copied().unwrap_or(false) {
        return;
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return;
    }

    if is_batch {
        if let Some(primary_value) = primary_value {
            if trimmed == primary_value.trim() {
                return;
            }
        }
    }

    if trimmed.is_empty() {
        *destination = None;
    } else {
        *destination = Some(trimmed.to_string());
    }
}

fn parse_u32_mixed(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    current: Option<u32>,
    label: &'static str,
    errors: &mut Vec<&'static str>,
) -> Result<Option<u32>, String> {
    if state.inspector_mixed.get(&field).copied().unwrap_or(false) {
        return Ok(current);
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return Ok(current);
    }

    if trimmed.is_empty() {
        return Ok(None);
    }

    let value = parse_optional_u32(trimmed)
        .inspect_err(|_| errors.push(label))
        .ok()
        .flatten();

    Ok(value)
}

fn parse_release_date_mixed(
    state: &Sonora,
    input: &str,
    current: Option<String>,
    errors: &mut Vec<&'static str>,
) -> Result<Option<String>, String> {
    if state
        .inspector_mixed
        .get(&InspectorField::ReleaseDate)
        .copied()
        .unwrap_or(false)
    {
        return Ok(current);
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return Ok(current);
    }

    if trimmed.is_empty() {
        return Ok(None);
    }

    let value = parse_optional_release_date(trimmed)
        .inspect_err(|_| errors.push("Release Date"))
        .ok()
        .flatten();

    Ok(value)
}
