//! gui/view/inspector.rs
//! Right panel: metadata inspector/editor.

use iced::Alignment;
use iced::Length;
use iced::widget::{Column, Row};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, text_input};

use super::super::state::{InspectorField as Field, Message, Sonora};
use super::widgets::fmt_duration;

use super::constants::LABEL_W;

/// Field row that appends " (mixed)" to the label when mixed.
fn field_row_mixed<'a>(
    label: &'a str,
    value: &'a str,
    mixed: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> Row<'a, Message> {
    let label = if mixed {
        format!("{label} (mixed)")
    } else {
        label.to_string()
    };

    row![
        text(label).width(Length::Fixed(LABEL_W)),
        text_input("", value).on_input(on_input).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
}

/// Numeric pair row with " (mixed)" on the label if either side is mixed.
fn num_pair_row_mixed<'a>(
    label: &'a str,
    left: &'a str,
    left_mixed: bool,
    left_on: impl Fn(String) -> Message + 'a,
    right: &'a str,
    right_mixed: bool,
    right_on: impl Fn(String) -> Message + 'a,
) -> Row<'a, Message> {
    let label = if left_mixed || right_mixed {
        format!("{label} (mixed)")
    } else {
        label.to_string()
    };

    row![
        text(label).width(Length::Fixed(LABEL_W)),
        text_input("", left)
            .on_input(left_on)
            .width(Length::Fixed(70.0)),
        text("/"),
        text_input("", right)
            .on_input(right_on)
            .width(Length::Fixed(70.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
}

fn is_mixed(state: &Sonora, field: Field) -> bool {
    state.inspector_mixed.get(&field).copied().unwrap_or(false)
}

pub(crate) fn build_inspector_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    // If nothing selected, show empty editor prompt.
    if state.selected_tracks.is_empty() && state.selected_track.is_none() {
        return container(
            column![
                text("Metadata editor").size(18),
                text("Select one or more tracks (center panel)."),
            ]
            .spacing(8),
        )
        .padding(12);
    }

    // Primary index for the header/path: prefer selected_track, else first selected_tracks.
    let primary_idx = state
        .selected_track
        .or_else(|| state.selected_tracks.iter().next().copied());

    let Some(i) = primary_idx else {
        return container(text("No selection.")).padding(12);
    };

    if i >= state.tracks.len() {
        return container(text("Invalid selection (rescan?).")).padding(12);
    }

    let t = &state.tracks[i];
    let path_line = format!("{}", t.path.display());

    let sel_count = if !state.selected_tracks.is_empty() {
        state.selected_tracks.len()
    } else {
        1
    };

    let top = column![
        text("Metadata editor").size(18),
        text(format!("Selected: {sel_count}")).size(12),
        text("File path").size(12),
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

    // IMPORTANT FIX:
    // "mixed" label should reflect the selection, not whether the textbox currently equals "<keep>".
    let core: Column<'_, Message> = column![
        field_row_mixed(
            "Title",
            &state.inspector.title,
            is_mixed(state, Field::Title),
            |s| { Message::InspectorChanged(Field::Title, s) }
        ),
        field_row_mixed(
            "Artist",
            &state.inspector.artist,
            is_mixed(state, Field::Artist),
            |s| { Message::InspectorChanged(Field::Artist, s) }
        ),
        field_row_mixed(
            "Album",
            &state.inspector.album,
            is_mixed(state, Field::Album),
            |s| { Message::InspectorChanged(Field::Album, s) }
        ),
        field_row_mixed(
            "Album Artist",
            &state.inspector.album_artist,
            is_mixed(state, Field::AlbumArtist),
            |s| Message::InspectorChanged(Field::AlbumArtist, s)
        ),
        field_row_mixed(
            "Composer",
            &state.inspector.composer,
            is_mixed(state, Field::Composer),
            |s| Message::InspectorChanged(Field::Composer, s)
        ),
        num_pair_row_mixed(
            "Track",
            &state.inspector.track_no,
            is_mixed(state, Field::TrackNo),
            |s| Message::InspectorChanged(Field::TrackNo, s),
            &state.inspector.track_total,
            is_mixed(state, Field::TrackTotal),
            |s| Message::InspectorChanged(Field::TrackTotal, s),
        ),
        num_pair_row_mixed(
            "Disc",
            &state.inspector.disc_no,
            is_mixed(state, Field::DiscNo),
            |s| Message::InspectorChanged(Field::DiscNo, s),
            &state.inspector.disc_total,
            is_mixed(state, Field::DiscTotal),
            |s| Message::InspectorChanged(Field::DiscTotal, s),
        ),
        field_row_mixed(
            "Year",
            &state.inspector.year,
            is_mixed(state, Field::Year),
            |s| { Message::InspectorChanged(Field::Year, s) }
        ),
        field_row_mixed(
            "Genre",
            &state.inspector.genre,
            is_mixed(state, Field::Genre),
            |s| { Message::InspectorChanged(Field::Genre, s) }
        ),
        field_row_mixed(
            "Grouping",
            &state.inspector.grouping,
            is_mixed(state, Field::Grouping),
            |s| Message::InspectorChanged(Field::Grouping, s)
        ),
        field_row_mixed(
            "Comment",
            &state.inspector.comment,
            is_mixed(state, Field::Comment),
            |s| Message::InspectorChanged(Field::Comment, s)
        ),
        field_row_mixed(
            "Lyrics",
            &state.inspector.lyrics,
            is_mixed(state, Field::Lyrics),
            |s| Message::InspectorChanged(Field::Lyrics, s)
        ),
        field_row_mixed(
            "Lyricist",
            &state.inspector.lyricist,
            is_mixed(state, Field::Lyricist),
            |s| Message::InspectorChanged(Field::Lyricist, s)
        ),
    ]
    .spacing(8);

    // If this checkbox constructor doesn't compile in your iced version,
    // switch to: checkbox("Show more tags", state.show_extended).on_toggle(...)
    let toggle = checkbox(state.show_extended)
        .label("Show more tags")
        .on_toggle(Message::ToggleExtended);

    let extended: Column<'_, Message> = if state.show_extended {
        column![
            field_row_mixed(
                "Date",
                &state.inspector.date,
                is_mixed(state, Field::Date),
                |s| { Message::InspectorChanged(Field::Date, s) }
            ),
            field_row_mixed(
                "Conductor",
                &state.inspector.conductor,
                is_mixed(state, Field::Conductor),
                |s| Message::InspectorChanged(Field::Conductor, s)
            ),
            field_row_mixed(
                "Remixer",
                &state.inspector.remixer,
                is_mixed(state, Field::Remixer),
                |s| Message::InspectorChanged(Field::Remixer, s)
            ),
            field_row_mixed(
                "Publisher",
                &state.inspector.publisher,
                is_mixed(state, Field::Publisher),
                |s| Message::InspectorChanged(Field::Publisher, s)
            ),
            field_row_mixed(
                "Subtitle",
                &state.inspector.subtitle,
                is_mixed(state, Field::Subtitle),
                |s| Message::InspectorChanged(Field::Subtitle, s)
            ),
            field_row_mixed(
                "BPM",
                &state.inspector.bpm,
                is_mixed(state, Field::Bpm),
                |s| { Message::InspectorChanged(Field::Bpm, s) }
            ),
            field_row_mixed(
                "Key",
                &state.inspector.key,
                is_mixed(state, Field::Key),
                |s| { Message::InspectorChanged(Field::Key, s) }
            ),
            field_row_mixed(
                "Mood",
                &state.inspector.mood,
                is_mixed(state, Field::Mood),
                |s| Message::InspectorChanged(Field::Mood, s)
            ),
            field_row_mixed(
                "Language",
                &state.inspector.language,
                is_mixed(state, Field::Language),
                |s| Message::InspectorChanged(Field::Language, s)
            ),
            field_row_mixed(
                "ISRC",
                &state.inspector.isrc,
                is_mixed(state, Field::Isrc),
                |s| { Message::InspectorChanged(Field::Isrc, s) }
            ),
            field_row_mixed(
                "Encoder",
                &state.inspector.encoder_settings,
                is_mixed(state, Field::EncoderSettings),
                |s| Message::InspectorChanged(Field::EncoderSettings, s)
            ),
            field_row_mixed(
                "Encoded by",
                &state.inspector.encoded_by,
                is_mixed(state, Field::EncodedBy),
                |s| Message::InspectorChanged(Field::EncodedBy, s)
            ),
            field_row_mixed(
                "Copyright",
                &state.inspector.copyright,
                is_mixed(state, Field::Copyright),
                |s| Message::InspectorChanged(Field::Copyright, s)
            ),
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
