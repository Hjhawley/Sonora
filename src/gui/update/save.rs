//! gui/update/save.rs
//!
//! Inspector save pipeline.
//!
//! Core guarantees:
//! - Save targets are identified by TrackId.
//! - Only fields explicitly edited by the user are applied.
//! - Untouched mixed and display-only values are preserved.
//! - Track and disc counter formatting is retained exactly.
//! - Artwork-only saves do not rewrite textual metadata.
//! - Every file in a batch is attempted, even if another file fails.
//! - Successful file changes are re-read and retained after partial failure.
//! - Successfully refreshed rows are written back to SQLite.
//! - In-memory rows change only after a successful reread.

use std::path::PathBuf;

use iced::Task;
use iced::widget::image;

use super::super::state::{
    ArtworkEdit, InspectorField, Message, SaveFileOutcome, SaveReport, SavedArtworkAction, Sonora,
    is_mixed_display_value,
};
use super::super::util::{
    extract_year_from_release_date, parse_optional_release_date, parse_optional_u32,
};
use super::inspector::load_inspector_from_selection;
use super::util::spawn_blocking;
use crate::core::types::{TrackId, TrackRow};

#[derive(Debug, Clone)]
struct CounterValue {
    text: String,
    numeric: u32,
}

#[derive(Debug, Default, Clone)]
struct InspectorPatch {
    title: Option<Option<String>>,
    artist: Option<Option<String>>,
    album: Option<Option<String>>,
    album_artist: Option<Option<String>>,
    composer: Option<Option<String>>,

    track_no: Option<Option<CounterValue>>,
    track_total: Option<Option<CounterValue>>,
    disc_no: Option<Option<CounterValue>>,
    disc_total: Option<Option<CounterValue>>,

    release_date: Option<Option<String>>,
    genre: Option<Option<String>>,

    grouping: Option<Option<String>>,
    content_group: Option<Option<String>>,
    comment: Option<Option<String>>,
    lyrics: Option<Option<String>>,
    lyricist: Option<Option<String>>,

    conductor: Option<Option<String>>,
    remixer: Option<Option<String>>,
    publisher: Option<Option<String>>,
    subtitle: Option<Option<String>>,
    bpm: Option<Option<u32>>,
    key: Option<Option<String>>,
    mood: Option<Option<String>>,
    language: Option<Option<String>>,
    isrc: Option<Option<String>>,
    encoder_settings: Option<Option<String>>,
    encoded_by: Option<Option<String>>,
    copyright: Option<Option<String>>,
}

impl InspectorPatch {
    fn apply_to(&self, row: &mut TrackRow) {
        apply_text_change(&self.title, &mut row.title);
        apply_text_change(&self.artist, &mut row.artist);
        apply_text_change(&self.album, &mut row.album);
        apply_text_change(&self.album_artist, &mut row.album_artist);
        apply_text_change(&self.composer, &mut row.composer);

        apply_counter_change(&self.track_no, &mut row.track_no_text, &mut row.track_no);

        apply_counter_change(
            &self.track_total,
            &mut row.track_total_text,
            &mut row.track_total,
        );

        apply_counter_change(&self.disc_no, &mut row.disc_no_text, &mut row.disc_no);

        apply_counter_change(
            &self.disc_total,
            &mut row.disc_total_text,
            &mut row.disc_total,
        );

        if let Some(value) = &self.release_date {
            row.release_date = value.clone();
            row.year = extract_year_from_release_date(row.release_date.as_deref());
        }

        apply_text_change(&self.genre, &mut row.genre);

        apply_text_change(&self.grouping, &mut row.grouping);
        apply_text_change(&self.content_group, &mut row.content_group);
        apply_text_change(&self.comment, &mut row.comment);
        apply_text_change(&self.lyrics, &mut row.lyrics);
        apply_text_change(&self.lyricist, &mut row.lyricist);

        apply_text_change(&self.conductor, &mut row.conductor);
        apply_text_change(&self.remixer, &mut row.remixer);
        apply_text_change(&self.publisher, &mut row.publisher);
        apply_text_change(&self.subtitle, &mut row.subtitle);

        if let Some(value) = self.bpm {
            row.bpm = value;
        }

        apply_text_change(&self.key, &mut row.key);
        apply_text_change(&self.mood, &mut row.mood);
        apply_text_change(&self.language, &mut row.language);
        apply_text_change(&self.isrc, &mut row.isrc);
        apply_text_change(&self.encoder_settings, &mut row.encoder_settings);
        apply_text_change(&self.encoded_by, &mut row.encoded_by);
        apply_text_change(&self.copyright, &mut row.copyright);
    }
}

#[derive(Debug, Clone)]
struct SaveTarget {
    id: TrackId,
    path: PathBuf,
    cached_row: TrackRow,
}

#[derive(Debug, Clone)]
enum PendingArtwork {
    Unchanged,
    Remove,
    Replace { bytes: Vec<u8>, mime: String },
}

impl PendingArtwork {
    fn from_edit(edit: &ArtworkEdit) -> Self {
        match edit {
            ArtworkEdit::Unchanged => Self::Unchanged,
            ArtworkEdit::Remove => Self::Remove,
            ArtworkEdit::Replace { bytes, mime, .. } => Self::Replace {
                bytes: bytes.clone(),
                mime: mime.clone(),
            },
        }
    }

    #[inline]
    fn is_requested(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }

    fn saved_action(&self) -> SavedArtworkAction {
        match self {
            Self::Unchanged => SavedArtworkAction::Unchanged,
            Self::Remove => SavedArtworkAction::Remove,
            Self::Replace { bytes, .. } => SavedArtworkAction::Replace {
                preview_bytes: bytes.clone(),
            },
        }
    }
}

pub(crate) fn save_inspector_to_file(state: &mut Sonora) -> Task<Message> {
    if state.scanning || state.saving {
        return Task::none();
    }

    state.refresh_inspector_dirty();

    if !state.inspector_dirty {
        state.status = "No changes to save.".to_string();
        return Task::none();
    }

    let ids = selected_ids_for_save(state);

    if ids.is_empty() {
        state.status = "Select a track first.".to_string();
        return Task::none();
    }

    let metadata_requested = !state.inspector_touched_fields.is_empty();

    let patch = match build_inspector_patch(state) {
        Ok(patch) => patch,
        Err(error) => {
            state.status = error;
            return Task::none();
        }
    };

    let artwork = PendingArtwork::from_edit(&state.inspector_art_edit);

    if !metadata_requested && !artwork.is_requested() {
        state.status = "No changes to save.".to_string();
        state.refresh_inspector_dirty();
        return Task::none();
    }

    let mut targets: Vec<SaveTarget> = Vec::with_capacity(ids.len());

    for id in ids {
        let row = match state.track_by_id(id).cloned() {
            Some(row) => row,
            None => {
                state.status = format!("Track {id} is no longer available. Reload the library.");
                return Task::none();
            }
        };

        targets.push(SaveTarget {
            id,
            path: row.path.clone(),
            cached_row: row,
        });
    }

    let requested = targets.len();

    state.saving = true;

    if requested == 1 {
        state.status = "Writing changes to file...".to_string();
    } else {
        state.status = format!("Writing changes to {requested} files...");
    }

    Task::perform(
        spawn_blocking(move || run_save_job(targets, patch, metadata_requested, artwork)),
        Message::SaveCompleted,
    )
}

pub(crate) fn save_completed(state: &mut Sonora, report: SaveReport) -> Task<Message> {
    state.saving = false;

    for file in &report.files {
        match file.refreshed_row.as_ref() {
            Some(refreshed_row) => {
                if let Some(slot) = state.track_by_id_mut(file.id) {
                    *slot = refreshed_row.clone();
                }
            }
            None => {}
        }
    }

    apply_saved_artwork_to_cache(state, &report);
    state.rebuild_library_caches();

    let mut complete_count: usize = 0;

    for file in &report.files {
        if file_completed_without_error(file) {
            complete_count += 1;
        }
    }

    let all_files_complete =
        report.files.len() == report.requested && complete_count == report.requested;

    if all_files_complete {
        load_inspector_from_selection(state);

        let mut status = String::new();

        if report.requested == 1 {
            status.push_str("Changes saved to file.");
        } else {
            status.push_str(&format!("Changes saved to {} files.", report.requested));
        }

        match report.db_error.as_ref() {
            Some(db_error) => {
                status.push_str(&format!(
                    " The files were updated, but Sonora could not fully update its library \
                     cache: {db_error}. Run Scan before restarting."
                ));
            }
            None => {}
        }

        state.status = status;
    } else {
        state.refresh_inspector_dirty();

        let mut first_error = "An unknown save error occurred.".to_string();

        'error_search: for file in &report.files {
            for error in &file.errors {
                first_error = format!("{}: {error}", file.path.display());
                break 'error_search;
            }
        }

        let mut status = format!(
            "Save partially completed: {complete_count} of {} files finished without errors. \
             Successful changes were kept. First error: {first_error}",
            report.requested
        );

        match report.db_error.as_ref() {
            Some(db_error) => {
                status.push_str(&format!(
                    " Library-cache update also failed: {db_error}. Run Scan before restarting."
                ));
            }
            None => {}
        }

        state.status = status;
    }

    Task::none()
}

fn run_save_job(
    targets: Vec<SaveTarget>,
    patch: InspectorPatch,
    metadata_requested: bool,
    artwork: PendingArtwork,
) -> SaveReport {
    let requested = targets.len();
    let artwork_action = artwork.saved_action();

    let mut files: Vec<SaveFileOutcome> = Vec::with_capacity(requested);

    for target in targets {
        files.push(save_one_file(target, &patch, metadata_requested, &artwork));
    }

    let refreshed_rows: Vec<TrackRow> = files
        .iter()
        .filter_map(|file| file.refreshed_row.clone())
        .collect();

    let db_error = if refreshed_rows.is_empty() {
        None
    } else {
        persist_refreshed_rows(&refreshed_rows).err()
    };

    SaveReport {
        requested,
        files,
        artwork_action,
        db_error,
    }
}

fn save_one_file(
    target: SaveTarget,
    patch: &InspectorPatch,
    metadata_requested: bool,
    artwork: &PendingArtwork,
) -> SaveFileOutcome {
    let mut errors: Vec<String> = Vec::new();

    let mut metadata_succeeded = false;
    let mut artwork_succeeded = false;

    if metadata_requested {
        let (mut current_row, tag_read_failed) =
            crate::core::tags::read_track_row(target.path.clone());

        if tag_read_failed {
            current_row = target.cached_row.clone();
        }

        current_row.id = Some(target.id);
        patch.apply_to(&mut current_row);

        match crate::core::tags::write_track_row(&current_row, true) {
            Ok(()) => {
                metadata_succeeded = true;
            }
            Err(error) => {
                errors.push(format!("metadata write failed: {error}"));
            }
        }
    }

    match artwork {
        PendingArtwork::Unchanged => {}

        PendingArtwork::Remove => match crate::core::tags::remove_embedded_art(&target.path) {
            Ok(_) => {
                artwork_succeeded = true;
            }
            Err(error) => {
                errors.push(format!("artwork removal failed: {error}"));
            }
        },

        PendingArtwork::Replace { bytes, mime } => {
            match crate::core::tags::set_embedded_art(&target.path, bytes, mime) {
                Ok(()) => {
                    artwork_succeeded = true;
                }
                Err(error) => {
                    errors.push(format!("artwork replacement failed: {error}"));
                }
            }
        }
    }

    let artwork_attempted = artwork.is_requested();
    let any_change_reached_disk = metadata_succeeded || artwork_succeeded;

    let refreshed_row = if any_change_reached_disk {
        let (mut row, reread_failed) = crate::core::tags::read_track_row(target.path.clone());

        if reread_failed {
            errors.push("changes reached disk, but metadata reread failed".to_string());
            None
        } else {
            row.id = Some(target.id);
            Some(row)
        }
    } else {
        None
    };

    SaveFileOutcome {
        id: target.id,
        path: target.path,
        refreshed_row,

        metadata_attempted: metadata_requested,
        metadata_succeeded,

        artwork_attempted,
        artwork_succeeded,

        errors,
    }
}

fn persist_refreshed_rows(rows: &[TrackRow]) -> Result<(), String> {
    let db_path = crate::core::db::default_db_path()?;
    let mut db = crate::core::db::Db::open(&db_path)?;

    match db.update_track_rows_metadata(rows) {
        Ok(()) => Ok(()),

        Err(batch_error) => {
            let mut individual_failures: Vec<String> = Vec::new();

            for row in rows {
                if let Err(error) = db.update_track_rows_metadata(std::slice::from_ref(row)) {
                    let id = row
                        .id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "<missing id>".to_string());

                    individual_failures.push(format!("track {id}: {error}"));
                }
            }

            if individual_failures.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "{batch_error}; individual retry failures: {}",
                    individual_failures.join(" | ")
                ))
            }
        }
    }
}

fn file_completed_without_error(file: &SaveFileOutcome) -> bool {
    let metadata_complete = !file.metadata_attempted || file.metadata_succeeded;
    let artwork_complete = !file.artwork_attempted || file.artwork_succeeded;

    metadata_complete && artwork_complete && file.refreshed_row.is_some() && file.errors.is_empty()
}

fn apply_saved_artwork_to_cache(state: &mut Sonora, report: &SaveReport) {
    let successful_artwork_ids: Vec<TrackId> = report
        .files
        .iter()
        .filter(|file| file.artwork_attempted && file.artwork_succeeded)
        .map(|file| file.id)
        .collect();

    if successful_artwork_ids.is_empty() {
        return;
    }

    match &report.artwork_action {
        SavedArtworkAction::Unchanged => {}

        SavedArtworkAction::Remove => {
            for id in successful_artwork_ids {
                state.cover_cache.remove(&id);
            }
        }

        SavedArtworkAction::Replace { preview_bytes } => {
            let preview = image::Handle::from_bytes(preview_bytes.clone());

            for id in successful_artwork_ids {
                state.cover_cache.insert(id, preview.clone());
            }
        }
    }
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

fn build_inspector_patch(state: &Sonora) -> Result<InspectorPatch, String> {
    Ok(InspectorPatch {
        title: text_change(
            state,
            InspectorField::Title,
            &state.inspector.title,
            "Title",
        )?,
        artist: text_change(
            state,
            InspectorField::Artist,
            &state.inspector.artist,
            "Artist",
        )?,
        album: text_change(
            state,
            InspectorField::Album,
            &state.inspector.album,
            "Album",
        )?,
        album_artist: text_change(
            state,
            InspectorField::AlbumArtist,
            &state.inspector.album_artist,
            "Album Artist",
        )?,
        composer: text_change(
            state,
            InspectorField::Composer,
            &state.inspector.composer,
            "Composer",
        )?,

        track_no: counter_change(
            state,
            InspectorField::TrackNo,
            &state.inspector.track_no,
            "Track #",
        )?,
        track_total: counter_change(
            state,
            InspectorField::TrackTotal,
            &state.inspector.track_total,
            "Track total",
        )?,
        disc_no: counter_change(
            state,
            InspectorField::DiscNo,
            &state.inspector.disc_no,
            "Disc #",
        )?,
        disc_total: counter_change(
            state,
            InspectorField::DiscTotal,
            &state.inspector.disc_total,
            "Disc total",
        )?,

        release_date: release_date_change(state)?,
        genre: text_change(
            state,
            InspectorField::Genre,
            &state.inspector.genre,
            "Genre",
        )?,

        grouping: text_change(
            state,
            InspectorField::Grouping,
            &state.inspector.grouping,
            "Grouping",
        )?,
        content_group: text_change(
            state,
            InspectorField::ContentGroup,
            &state.inspector.content_group,
            "Content Group",
        )?,
        comment: text_change(
            state,
            InspectorField::Comment,
            &state.inspector.comment,
            "Comment",
        )?,
        lyrics: text_change(
            state,
            InspectorField::Lyrics,
            &state.inspector.lyrics,
            "Lyrics",
        )?,
        lyricist: text_change(
            state,
            InspectorField::Lyricist,
            &state.inspector.lyricist,
            "Lyricist",
        )?,

        conductor: text_change(
            state,
            InspectorField::Conductor,
            &state.inspector.conductor,
            "Conductor",
        )?,
        remixer: text_change(
            state,
            InspectorField::Remixer,
            &state.inspector.remixer,
            "Remixer",
        )?,
        publisher: text_change(
            state,
            InspectorField::Publisher,
            &state.inspector.publisher,
            "Publisher",
        )?,
        subtitle: text_change(
            state,
            InspectorField::Subtitle,
            &state.inspector.subtitle,
            "Subtitle",
        )?,
        bpm: u32_change(state, InspectorField::Bpm, &state.inspector.bpm, "BPM")?,
        key: text_change(state, InspectorField::Key, &state.inspector.key, "Key")?,
        mood: text_change(state, InspectorField::Mood, &state.inspector.mood, "Mood")?,
        language: text_change(
            state,
            InspectorField::Language,
            &state.inspector.language,
            "Language",
        )?,
        isrc: text_change(state, InspectorField::Isrc, &state.inspector.isrc, "ISRC")?,
        encoder_settings: text_change(
            state,
            InspectorField::EncoderSettings,
            &state.inspector.encoder_settings,
            "Encoder",
        )?,
        encoded_by: text_change(
            state,
            InspectorField::EncodedBy,
            &state.inspector.encoded_by,
            "Encoded by",
        )?,
        copyright: text_change(
            state,
            InspectorField::Copyright,
            &state.inspector.copyright,
            "Copyright",
        )?,
    })
}

fn text_change(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    label: &str,
) -> Result<Option<Option<String>>, String> {
    if !state.inspector_touched_fields.contains(&field) {
        return Ok(None);
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return Err(format!(
            "Not saved: {label} still contains the mixed-value placeholder."
        ));
    }

    if trimmed.is_empty() {
        Ok(Some(None))
    } else {
        Ok(Some(Some(trimmed.to_string())))
    }
}

fn counter_change(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    label: &str,
) -> Result<Option<Option<CounterValue>>, String> {
    if !state.inspector_touched_fields.contains(&field) {
        return Ok(None);
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return Err(format!(
            "Not saved: {label} still contains the mixed-value placeholder."
        ));
    }

    if trimmed.is_empty() {
        return Ok(Some(None));
    }

    if !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("Not saved: invalid {label}."));
    }

    let numeric = trimmed
        .parse::<u32>()
        .map_err(|_| format!("Not saved: invalid {label}."))?;

    Ok(Some(Some(CounterValue {
        text: trimmed.to_string(),
        numeric,
    })))
}

fn u32_change(
    state: &Sonora,
    field: InspectorField,
    input: &str,
    label: &str,
) -> Result<Option<Option<u32>>, String> {
    if !state.inspector_touched_fields.contains(&field) {
        return Ok(None);
    }

    let trimmed = input.trim();

    if is_mixed_display_value(trimmed) {
        return Err(format!(
            "Not saved: {label} still contains the mixed-value placeholder."
        ));
    }

    if trimmed.is_empty() {
        return Ok(Some(None));
    }

    parse_optional_u32(trimmed)
        .map(Some)
        .map_err(|_| format!("Not saved: invalid {label}."))
}

fn release_date_change(state: &Sonora) -> Result<Option<Option<String>>, String> {
    if !state
        .inspector_touched_fields
        .contains(&InspectorField::ReleaseDate)
    {
        return Ok(None);
    }

    let trimmed = state.inspector.release_date.trim();

    if is_mixed_display_value(trimmed) {
        return Err(
            "Not saved: Release Date still contains the mixed-value placeholder.".to_string(),
        );
    }

    if trimmed.is_empty() {
        return Ok(Some(None));
    }

    parse_optional_release_date(trimmed)
        .map(Some)
        .map_err(|_| "Not saved: invalid Release Date.".to_string())
}

fn apply_text_change(change: &Option<Option<String>>, destination: &mut Option<String>) {
    if let Some(value) = change {
        *destination = value.clone();
    }
}

fn apply_counter_change(
    change: &Option<Option<CounterValue>>,
    text_destination: &mut Option<String>,
    numeric_destination: &mut Option<u32>,
) {
    let Some(change) = change else {
        return;
    };

    match change {
        Some(value) => {
            *text_destination = Some(value.text.clone());
            *numeric_destination = Some(value.numeric);
        }

        None => {
            *text_destination = None;
            *numeric_destination = None;
        }
    }
}
