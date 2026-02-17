use iced::Task;

use super::super::state::{InspectorField, Message, Sonora};
use super::super::util::filename_stem;

pub(crate) fn toggle_extended(state: &mut Sonora, v: bool) -> Task<Message> {
    state.show_extended = v;
    Task::none()
}

pub(crate) fn inspector_changed(
    state: &mut Sonora,
    field: InspectorField,
    value: String,
) -> Task<Message> {
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

pub(crate) fn load_inspector_from_track(state: &mut Sonora) {
    let Some(i) = state.selected_track else {
        clear_inspector(state);
        return;
    };
    if i >= state.tracks.len() {
        clear_inspector(state);
        return;
    }

    let t = &state.tracks[i];

    // Standard (visible by default)
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
    state.inspector.genre = t.genre.clone().unwrap_or_default();

    state.inspector.grouping = t.grouping.clone().unwrap_or_default();
    state.inspector.comment = t.comment.clone().unwrap_or_default();
    state.inspector.lyrics = t.lyrics.clone().unwrap_or_default();
    state.inspector.lyricist = t.lyricist.clone().unwrap_or_default();

    // Extended (toggleable)
    state.inspector.date = t.date.clone().unwrap_or_default();
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

pub(crate) fn clear_inspector(state: &mut Sonora) {
    state.inspector = Default::default();
    state.inspector_dirty = false;
}
