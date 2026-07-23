//! gui/update/inspector.rs
//!
//! Inspector draft state machine + mixed-selection semantics.
//! - Selection is stored as TrackId(s).
//! - We resolve ids -> indices only when we need to read TrackRow(s).
//! - Mixed-state is tracked structurally in 'state.inspector_mixed'.
//! - The draft may display '<mixed>', but that is only a UI placeholder.

use std::collections::BTreeMap;

use iced::Task;

use super::super::state::{InspectorDraft, InspectorField, Message, Sonora};
use super::super::util::filename_stem;
use super::art::reset_inspector_artwork_state;
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

pub(crate) fn close_inspector(state: &mut Sonora) -> Task<Message> {
    state.inspector_open = false;

    if state.inspector_dirty {
        clear_inspector(state);
    }

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

        InspectorField::ReleaseDate => state.inspector.release_date = value,
        InspectorField::Genre => state.inspector.genre = value,

        InspectorField::Grouping => state.inspector.grouping = value,
        InspectorField::ContentGroup => state.inspector.content_group = value,
        InspectorField::Comment => state.inspector.comment = value,
        InspectorField::Lyrics => state.inspector.lyrics = value,
        InspectorField::Lyricist => state.inspector.lyricist = value,

        InspectorField::Conductor => state.inspector.conductor = value,
        InspectorField::Remixer => state.inspector.remixer = value,
        InspectorField::Publisher => state.inspector.publisher = value,
        InspectorField::EncoderSettings => state.inspector.encoder_settings = value,
        InspectorField::EncodedBy => state.inspector.encoded_by = value,
        InspectorField::Subtitle => state.inspector.subtitle = value,
        InspectorField::Bpm => state.inspector.bpm = value,
        InspectorField::Key => state.inspector.key = value,
        InspectorField::Mood => state.inspector.mood = value,
        InspectorField::Language => state.inspector.language = value,
        InspectorField::Isrc => state.inspector.isrc = value,
        InspectorField::Copyright => state.inspector.copyright = value,
    }
}

pub(crate) fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
    state.inspector_mixed.clear();
    reset_inspector_artwork_state(state);
}

/// Load inspector fields from the current selection.
/// - Works for single-track and multi-track selection.
/// - Writes '<mixed>' into fields that disagree across selected tracks.
pub(crate) fn load_inspector_from_selection(state: &mut Sonora) {
    reset_inspector_artwork_state(state);

    let mut ids: Vec<TrackId> = Vec::new();

    if !state.selected_tracks.is_empty() {
        ids.extend(state.selected_tracks.iter().copied());
    } else if let Some(id) = state.selected_track {
        ids.push(id);
    } else {
        state.inspector_open = false;
        clear_inspector(state);
        return;
    }

    let idxs: Vec<usize> = ids
        .into_iter()
        .filter_map(|id| state.index_of_id(id))
        .collect();

    if idxs.is_empty() {
        state.inspector_open = false;
        clear_inspector(state);
        return;
    }

    state.inspector_open = true;

    fn opt_str(value: &Option<String>) -> String {
        value.clone().unwrap_or_default()
    }

    fn opt_u32(value: Option<u32>) -> String {
        value.map(|number| number.to_string()).unwrap_or_default()
    }

    fn apply_field(
        draft_slot: &mut String,
        mixed_map: &mut BTreeMap<InspectorField, bool>,
        field: InspectorField,
        values: Vec<String>,
    ) {
        let first = values.first().cloned().unwrap_or_default();
        let mixed = values.iter().any(|value| *value != first);

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
        .map(|&index| {
            state.tracks[index]
                .title
                .clone()
                .unwrap_or_else(|| filename_stem(&state.tracks[index].path))
        })
        .collect();

    let artists: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].artist))
        .collect();

    let albums: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].album))
        .collect();

    let album_artists: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].album_artist))
        .collect();

    let composers: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].composer))
        .collect();

    let track_no: Vec<String> = idxs
        .iter()
        .map(|&index| opt_u32(state.tracks[index].track_no))
        .collect();

    let track_total: Vec<String> = idxs
        .iter()
        .map(|&index| opt_u32(state.tracks[index].track_total))
        .collect();

    let disc_no: Vec<String> = idxs
        .iter()
        .map(|&index| opt_u32(state.tracks[index].disc_no))
        .collect();

    let disc_total: Vec<String> = idxs
        .iter()
        .map(|&index| opt_u32(state.tracks[index].disc_total))
        .collect();

    let release_dates: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].release_date))
        .collect();

    let genres: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].genre))
        .collect();

    let groupings: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].grouping))
        .collect();

    let content_groups: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].content_group))
        .collect();

    let comments: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].comment))
        .collect();

    let lyrics: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].lyrics))
        .collect();

    let lyricists: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].lyricist))
        .collect();

    let conductors: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].conductor))
        .collect();

    let remixers: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].remixer))
        .collect();

    let publishers: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].publisher))
        .collect();

    let subtitles: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].subtitle))
        .collect();

    let bpms: Vec<String> = idxs
        .iter()
        .map(|&index| opt_u32(state.tracks[index].bpm))
        .collect();

    let keys: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].key))
        .collect();

    let moods: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].mood))
        .collect();

    let languages: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].language))
        .collect();

    let isrcs: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].isrc))
        .collect();

    let encoder_settings: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].encoder_settings))
        .collect();

    let encoded_by: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].encoded_by))
        .collect();

    let copyrights: Vec<String> = idxs
        .iter()
        .map(|&index| opt_str(&state.tracks[index].copyright))
        .collect();

    let mut mixed_map: BTreeMap<InspectorField, bool> = BTreeMap::new();

    apply_field(
        &mut state.inspector.title,
        &mut mixed_map,
        InspectorField::Title,
        titles,
    );

    apply_field(
        &mut state.inspector.artist,
        &mut mixed_map,
        InspectorField::Artist,
        artists,
    );

    apply_field(
        &mut state.inspector.album,
        &mut mixed_map,
        InspectorField::Album,
        albums,
    );

    apply_field(
        &mut state.inspector.album_artist,
        &mut mixed_map,
        InspectorField::AlbumArtist,
        album_artists,
    );

    apply_field(
        &mut state.inspector.composer,
        &mut mixed_map,
        InspectorField::Composer,
        composers,
    );

    apply_field(
        &mut state.inspector.track_no,
        &mut mixed_map,
        InspectorField::TrackNo,
        track_no,
    );

    apply_field(
        &mut state.inspector.track_total,
        &mut mixed_map,
        InspectorField::TrackTotal,
        track_total,
    );

    apply_field(
        &mut state.inspector.disc_no,
        &mut mixed_map,
        InspectorField::DiscNo,
        disc_no,
    );

    apply_field(
        &mut state.inspector.disc_total,
        &mut mixed_map,
        InspectorField::DiscTotal,
        disc_total,
    );

    apply_field(
        &mut state.inspector.release_date,
        &mut mixed_map,
        InspectorField::ReleaseDate,
        release_dates,
    );

    apply_field(
        &mut state.inspector.genre,
        &mut mixed_map,
        InspectorField::Genre,
        genres,
    );

    apply_field(
        &mut state.inspector.grouping,
        &mut mixed_map,
        InspectorField::Grouping,
        groupings,
    );

    apply_field(
        &mut state.inspector.content_group,
        &mut mixed_map,
        InspectorField::ContentGroup,
        content_groups,
    );

    apply_field(
        &mut state.inspector.comment,
        &mut mixed_map,
        InspectorField::Comment,
        comments,
    );

    apply_field(
        &mut state.inspector.lyrics,
        &mut mixed_map,
        InspectorField::Lyrics,
        lyrics,
    );

    apply_field(
        &mut state.inspector.lyricist,
        &mut mixed_map,
        InspectorField::Lyricist,
        lyricists,
    );

    apply_field(
        &mut state.inspector.conductor,
        &mut mixed_map,
        InspectorField::Conductor,
        conductors,
    );

    apply_field(
        &mut state.inspector.remixer,
        &mut mixed_map,
        InspectorField::Remixer,
        remixers,
    );

    apply_field(
        &mut state.inspector.publisher,
        &mut mixed_map,
        InspectorField::Publisher,
        publishers,
    );

    apply_field(
        &mut state.inspector.encoder_settings,
        &mut mixed_map,
        InspectorField::EncoderSettings,
        encoder_settings,
    );

    apply_field(
        &mut state.inspector.encoded_by,
        &mut mixed_map,
        InspectorField::EncodedBy,
        encoded_by,
    );

    apply_field(
        &mut state.inspector.subtitle,
        &mut mixed_map,
        InspectorField::Subtitle,
        subtitles,
    );

    apply_field(
        &mut state.inspector.bpm,
        &mut mixed_map,
        InspectorField::Bpm,
        bpms,
    );

    apply_field(
        &mut state.inspector.key,
        &mut mixed_map,
        InspectorField::Key,
        keys,
    );

    apply_field(
        &mut state.inspector.mood,
        &mut mixed_map,
        InspectorField::Mood,
        moods,
    );

    apply_field(
        &mut state.inspector.language,
        &mut mixed_map,
        InspectorField::Language,
        languages,
    );

    apply_field(
        &mut state.inspector.isrc,
        &mut mixed_map,
        InspectorField::Isrc,
        isrcs,
    );

    apply_field(
        &mut state.inspector.copyright,
        &mut mixed_map,
        InspectorField::Copyright,
        copyrights,
    );

    state.inspector_mixed = mixed_map;
    state.inspector_dirty = false;
}
