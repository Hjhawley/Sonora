//! Right panel: metadata inspector/editor.

use iced::Length;
use iced::widget::{button, checkbox, column, container, row, scrollable, text};

use super::super::state::{InspectorField as Field, Message, Sonora};
use super::widgets::{field_row, fmt_duration, num_pair_row};

pub(crate) fn build_inspector_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let Some(i) = state.selected_track else {
        return container(
            column![
                text("Metadata editor").size(18),
                text("Select a track (center panel)."),
            ]
            .spacing(8),
        )
        .padding(12);
    };

    if i >= state.tracks.len() {
        return container(text("Invalid selection (rescan?).")).padding(12);
    }

    let t = &state.tracks[i];
    let path_line = format!("{}", t.path.display());

    let top = column![
        text("Metadata editor").size(18),
        text("Path").size(12),
        text(path_line).size(12),
        text(format!(
            "Artwork: {} | Len: {} | Rating: {} | Plays: {} | Compilation: {}",
            t.artwork_count,
            fmt_duration(t.duration_ms),
            t.rating
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
            t.play_count
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
            t.compilation
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into()),
        ))
        .size(12),
    ]
    .spacing(6);

    // Standard (visible by default)
    let core = column![
        field_row("Title", &state.inspector.title, |s| {
            Message::InspectorChanged(Field::Title, s)
        }),
        field_row("Artist", &state.inspector.artist, |s| {
            Message::InspectorChanged(Field::Artist, s)
        }),
        field_row("Album", &state.inspector.album, |s| {
            Message::InspectorChanged(Field::Album, s)
        }),
        field_row("Album Artist", &state.inspector.album_artist, |s| {
            Message::InspectorChanged(Field::AlbumArtist, s)
        }),
        field_row("Composer", &state.inspector.composer, |s| {
            Message::InspectorChanged(Field::Composer, s)
        }),
        num_pair_row(
            "Track",
            &state.inspector.track_no,
            |s| Message::InspectorChanged(Field::TrackNo, s),
            &state.inspector.track_total,
            |s| Message::InspectorChanged(Field::TrackTotal, s),
        ),
        num_pair_row(
            "Disc",
            &state.inspector.disc_no,
            |s| Message::InspectorChanged(Field::DiscNo, s),
            &state.inspector.disc_total,
            |s| Message::InspectorChanged(Field::DiscTotal, s),
        ),
        field_row("Year", &state.inspector.year, |s| {
            Message::InspectorChanged(Field::Year, s)
        }),
        field_row("Genre", &state.inspector.genre, |s| {
            Message::InspectorChanged(Field::Genre, s)
        }),
        field_row("Grouping", &state.inspector.grouping, |s| {
            Message::InspectorChanged(Field::Grouping, s)
        }),
        field_row("Comment", &state.inspector.comment, |s| {
            Message::InspectorChanged(Field::Comment, s)
        }),
        field_row("Lyrics", &state.inspector.lyrics, |s| {
            Message::InspectorChanged(Field::Lyrics, s)
        }),
        field_row("Lyricist", &state.inspector.lyricist, |s| {
            Message::InspectorChanged(Field::Lyricist, s)
        }),
    ]
    .spacing(8);

    let toggle = checkbox(state.show_extended)
        .label("Show more tags")
        .on_toggle(Message::ToggleExtended);

    // Extended (toggleable)
    let extended = if state.show_extended {
        column![
            field_row("Date", &state.inspector.date, |s| {
                Message::InspectorChanged(Field::Date, s)
            }),
            field_row("Conductor", &state.inspector.conductor, |s| {
                Message::InspectorChanged(Field::Conductor, s)
            }),
            field_row("Remixer", &state.inspector.remixer, |s| {
                Message::InspectorChanged(Field::Remixer, s)
            }),
            field_row("Publisher", &state.inspector.publisher, |s| {
                Message::InspectorChanged(Field::Publisher, s)
            }),
            field_row("Subtitle", &state.inspector.subtitle, |s| {
                Message::InspectorChanged(Field::Subtitle, s)
            }),
            field_row("BPM", &state.inspector.bpm, |s| {
                Message::InspectorChanged(Field::Bpm, s)
            }),
            field_row("Key", &state.inspector.key, |s| {
                Message::InspectorChanged(Field::Key, s)
            }),
            field_row("Mood", &state.inspector.mood, |s| {
                Message::InspectorChanged(Field::Mood, s)
            }),
            field_row("Language", &state.inspector.language, |s| {
                Message::InspectorChanged(Field::Language, s)
            }),
            field_row("ISRC", &state.inspector.isrc, |s| {
                Message::InspectorChanged(Field::Isrc, s)
            }),
            field_row("Encoder", &state.inspector.encoder_settings, |s| {
                Message::InspectorChanged(Field::EncoderSettings, s)
            }),
            field_row("Encoded by", &state.inspector.encoded_by, |s| {
                Message::InspectorChanged(Field::EncodedBy, s)
            }),
            field_row("Copyright", &state.inspector.copyright, |s| {
                Message::InspectorChanged(Field::Copyright, s)
            }),
        ]
        .spacing(8)
    } else {
        column![]
    };

    let save_btn = if state.scanning || !state.inspector_dirty {
        button("Save edits")
    } else {
        button("Save edits").on_press(Message::SaveInspectorToFile)
    };

    let revert_btn = if state.scanning {
        button("Cancel edits")
    } else {
        button("Cancel edits").on_press(Message::RevertInspector)
    };

    let buttons = row![save_btn, revert_btn].spacing(8);

    let editor = scrollable(column![top, core, toggle, extended].spacing(12)).height(Length::Fill);
    container(column![editor, buttons].spacing(12)).padding(12)
}
