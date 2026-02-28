//! gui/update/inspector.rs
//! Inspector draft state machine + mixed-selection semantics.
//!
//! - Selection is stored as TrackId(s).
//! - We resolve ids -> indices only when we need to read TrackRow(s).

use iced::Task;
use std::collections::BTreeMap;

use super::super::state::{InspectorField, KEEP_SENTINEL, Message, Sonora};
use super::super::util::filename_stem;
use crate::core::types::TrackId;

pub(crate) fn toggle_extended(state: &mut Sonora, v: bool) -> Task<Message> {
    state.show_extended = v;
    Task::none()
}

pub(crate) fn inspector_changed(
    state: &mut Sonora,
    field: InspectorField,
    value: String,
) -> Task<Message> {
    // If a field is currently mixed, editing should replace the sentinel with the new value
    // and clear the mixed flag for that field.
    if value != KEEP_SENTINEL {
        state.inspector_mixed.insert(field, false);
    }

    set_inspector_field(state, field, value);
    state.inspector_dirty = true;
    Task::none()
}

/// Update a single inspector string field based on `InspectorField`.
fn set_inspector_field(state: &mut Sonora, field: InspectorField, value: String) {
    match field {
        // Standard (visible by default)
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
        InspectorField::Genre => state.inspector.genre = value,

        InspectorField::Grouping => state.inspector.grouping = value,
        InspectorField::Comment => state.inspector.comment = value,
        InspectorField::Lyrics => state.inspector.lyrics = value,
        InspectorField::Lyricist => state.inspector.lyricist = value,

        // Extended (toggleable)
        InspectorField::Date => state.inspector.date = value,
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

pub(crate) fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
    state.inspector_mixed.clear();
}

/// Load inspector fields from the current selection.
/// - Works for single-track and multi-track selection.
/// - Writes KEEP_SENTINEL into fields that are mixed.
/// - Clears extended fields (for now) to avoid stale values.
pub(crate) fn load_inspector_from_selection(state: &mut Sonora) {
    // Determine which ids are selected
    let mut ids: Vec<TrackId> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(id) = state.selected_track {
        vec![id]
    } else {
        clear_inspector(state);
        return;
    };

    // Resolve ids -> indices (drop any stale ids)
    let mut idxs: Vec<usize> = ids
        .drain(..)
        .filter_map(|id| state.index_of_id(id))
        .collect();

    if idxs.is_empty() {
        clear_inspector(state);
        return;
    }

    // --- helpers ---
    fn opt_str(v: &Option<String>) -> String {
        v.clone().unwrap_or_default()
    }
    fn opt_u32(v: Option<u32>) -> String {
        v.map(|n| n.to_string()).unwrap_or_default()
    }
    fn opt_year_i32(v: Option<i32>) -> String {
        v.map(|y| y.to_string()).unwrap_or_default()
    }

    fn apply_field(
        draft_slot: &mut String,
        mixed_map: &mut BTreeMap<InspectorField, bool>,
        field: InspectorField,
        values: Vec<String>,
    ) {
        let first = values.first().cloned().unwrap_or_default();
        let mixed = values.iter().any(|v| *v != first);

        if mixed {
            *draft_slot = KEEP_SENTINEL.to_string();
            mixed_map.insert(field, true);
        } else {
            *draft_slot = first;
            mixed_map.insert(field, false);
        }
    }

    // Collect per-field strings for all selected tracks
    let titles: Vec<String> = idxs
        .iter()
        .map(|&i| {
            state.tracks[i]
                .title
                .clone()
                .unwrap_or_else(|| filename_stem(&state.tracks[i].path))
        })
        .collect();

    let artists: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].artist))
        .collect();
    let albums: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].album))
        .collect();
    let album_artists: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].album_artist))
        .collect();
    let composers: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].composer))
        .collect();

    let track_no: Vec<String> = idxs
        .iter()
        .map(|&i| opt_u32(state.tracks[i].track_no))
        .collect();
    let track_total: Vec<String> = idxs
        .iter()
        .map(|&i| opt_u32(state.tracks[i].track_total))
        .collect();
    let disc_no: Vec<String> = idxs
        .iter()
        .map(|&i| opt_u32(state.tracks[i].disc_no))
        .collect();
    let disc_total: Vec<String> = idxs
        .iter()
        .map(|&i| opt_u32(state.tracks[i].disc_total))
        .collect();

    let years: Vec<String> = idxs
        .iter()
        .map(|&i| opt_year_i32(state.tracks[i].year))
        .collect();
    let genres: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].genre))
        .collect();

    let grouping: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].grouping))
        .collect();
    let comment: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].comment))
        .collect();
    let lyrics: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].lyrics))
        .collect();
    let lyricist: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].lyricist))
        .collect();

    // Apply + compute mixed flags
    let mut map_mixed: BTreeMap<InspectorField, bool> = BTreeMap::new();

    apply_field(
        &mut state.inspector.title,
        &mut map_mixed,
        InspectorField::Title,
        titles,
    );
    apply_field(
        &mut state.inspector.artist,
        &mut map_mixed,
        InspectorField::Artist,
        artists,
    );
    apply_field(
        &mut state.inspector.album,
        &mut map_mixed,
        InspectorField::Album,
        albums,
    );
    apply_field(
        &mut state.inspector.album_artist,
        &mut map_mixed,
        InspectorField::AlbumArtist,
        album_artists,
    );
    apply_field(
        &mut state.inspector.composer,
        &mut map_mixed,
        InspectorField::Composer,
        composers,
    );

    apply_field(
        &mut state.inspector.track_no,
        &mut map_mixed,
        InspectorField::TrackNo,
        track_no,
    );
    apply_field(
        &mut state.inspector.track_total,
        &mut map_mixed,
        InspectorField::TrackTotal,
        track_total,
    );
    apply_field(
        &mut state.inspector.disc_no,
        &mut map_mixed,
        InspectorField::DiscNo,
        disc_no,
    );
    apply_field(
        &mut state.inspector.disc_total,
        &mut map_mixed,
        InspectorField::DiscTotal,
        disc_total,
    );

    apply_field(
        &mut state.inspector.year,
        &mut map_mixed,
        InspectorField::Year,
        years,
    );
    apply_field(
        &mut state.inspector.genre,
        &mut map_mixed,
        InspectorField::Genre,
        genres,
    );

    apply_field(
        &mut state.inspector.grouping,
        &mut map_mixed,
        InspectorField::Grouping,
        grouping,
    );
    apply_field(
        &mut state.inspector.comment,
        &mut map_mixed,
        InspectorField::Comment,
        comment,
    );
    apply_field(
        &mut state.inspector.lyrics,
        &mut map_mixed,
        InspectorField::Lyrics,
        lyrics,
    );
    apply_field(
        &mut state.inspector.lyricist,
        &mut map_mixed,
        InspectorField::Lyricist,
        lyricist,
    );

    state.inspector_mixed = map_mixed;

    // Avoid stale extended values until you implement mixed/aggregation for them.
    state.inspector.date.clear();
    state.inspector.conductor.clear();
    state.inspector.remixer.clear();
    state.inspector.publisher.clear();
    state.inspector.subtitle.clear();
    state.inspector.bpm.clear();
    state.inspector.key.clear();
    state.inspector.mood.clear();
    state.inspector.language.clear();
    state.inspector.isrc.clear();
    state.inspector.encoder_settings.clear();
    state.inspector.encoded_by.clear();
    state.inspector.copyright.clear();

    state.inspector_dirty = false;
}
