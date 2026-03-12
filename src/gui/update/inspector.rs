//! gui/update/inspector.rs
//! Inspector draft state machine + mixed-selection semantics.
//!
//! - Selection is stored as TrackId(s).
//! - We resolve ids -> indices only when we need to read TrackRow(s).
//! - Mixed-state is tracked structurally in `state.inspector_mixed`.
//! - The draft may display `<mixed>`, but that is only a UI placeholder.

use iced::Task;
use std::collections::BTreeMap;

use super::super::state::{InspectorDraft, InspectorField, Message, Sonora};
use super::super::util::filename_stem;
use crate::core::types::TrackId;

pub(crate) fn inspector_changed(
    state: &mut Sonora,
    field: InspectorField,
    value: String,
) -> Task<Message> {
    state.inspector_mixed.insert(field, false);
    set_inspector_field(state, field, value);
    state.inspector_dirty = true;
    Task::none()
}

fn set_inspector_field(state: &mut Sonora, field: InspectorField, value: String) {
    match field {
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
/// - Writes `<mixed>` into fields that disagree across selected tracks.
pub(crate) fn load_inspector_from_selection(state: &mut Sonora) {
    let mut ids: Vec<TrackId> = if !state.selected_tracks.is_empty() {
        state.selected_tracks.iter().copied().collect()
    } else if let Some(id) = state.selected_track {
        vec![id]
    } else {
        clear_inspector(state);
        return;
    };

    let idxs: Vec<usize> = ids
        .drain(..)
        .filter_map(|id| state.index_of_id(id))
        .collect();

    if idxs.is_empty() {
        clear_inspector(state);
        return;
    }

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
            InspectorDraft::set_mixed(draft_slot);
            mixed_map.insert(field, true);
        } else {
            *draft_slot = first;
            mixed_map.insert(field, false);
        }
    }

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

    let dates: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].date))
        .collect();
    let conductors: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].conductor))
        .collect();
    let remixers: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].remixer))
        .collect();
    let publishers: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].publisher))
        .collect();
    let subtitles: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].subtitle))
        .collect();
    let bpms: Vec<String> = idxs.iter().map(|&i| opt_u32(state.tracks[i].bpm)).collect();
    let keys: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].key))
        .collect();
    let moods: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].mood))
        .collect();
    let languages: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].language))
        .collect();
    let isrcs: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].isrc))
        .collect();
    let encoder_settings: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].encoder_settings))
        .collect();
    let encoded_by: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].encoded_by))
        .collect();
    let copyrights: Vec<String> = idxs
        .iter()
        .map(|&i| opt_str(&state.tracks[i].copyright))
        .collect();

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

    apply_field(
        &mut state.inspector.date,
        &mut map_mixed,
        InspectorField::Date,
        dates,
    );
    apply_field(
        &mut state.inspector.conductor,
        &mut map_mixed,
        InspectorField::Conductor,
        conductors,
    );
    apply_field(
        &mut state.inspector.remixer,
        &mut map_mixed,
        InspectorField::Remixer,
        remixers,
    );
    apply_field(
        &mut state.inspector.publisher,
        &mut map_mixed,
        InspectorField::Publisher,
        publishers,
    );
    apply_field(
        &mut state.inspector.subtitle,
        &mut map_mixed,
        InspectorField::Subtitle,
        subtitles,
    );
    apply_field(
        &mut state.inspector.bpm,
        &mut map_mixed,
        InspectorField::Bpm,
        bpms,
    );
    apply_field(
        &mut state.inspector.key,
        &mut map_mixed,
        InspectorField::Key,
        keys,
    );
    apply_field(
        &mut state.inspector.mood,
        &mut map_mixed,
        InspectorField::Mood,
        moods,
    );
    apply_field(
        &mut state.inspector.language,
        &mut map_mixed,
        InspectorField::Language,
        languages,
    );
    apply_field(
        &mut state.inspector.isrc,
        &mut map_mixed,
        InspectorField::Isrc,
        isrcs,
    );
    apply_field(
        &mut state.inspector.encoder_settings,
        &mut map_mixed,
        InspectorField::EncoderSettings,
        encoder_settings,
    );
    apply_field(
        &mut state.inspector.encoded_by,
        &mut map_mixed,
        InspectorField::EncodedBy,
        encoded_by,
    );
    apply_field(
        &mut state.inspector.copyright,
        &mut map_mixed,
        InspectorField::Copyright,
        copyrights,
    );

    state.inspector_mixed = map_mixed;
    state.inspector_dirty = false;
}
