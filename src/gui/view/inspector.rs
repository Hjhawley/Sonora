//! gui/view/inspector.rs
//! Right panel: metadata inspector/editor.
//! - Selection is TrackId-based.
//! - We resolve id -> TrackRow on demand for display.
//!
//! Phase-1 visual structure pass:
//! - stronger grouping
//! - clearer section rhythm
//! - more deliberate action area
//! - better unsaved-changes signaling
//!
//! The metadata remains intentionally thorough; the goal here is not to hide
//! capability, but to make the density feel structured instead of noisy.

use iced::widget::text_input::Status as TextInputStatus;
use iced::widget::{Column, Row, button, container, row, scrollable, text, text_input};
use iced::{Alignment, Color, Length, Theme};

use super::super::state::{
    ArtworkEdit, InspectorField as Field, Message, Sonora, mixed_display_string,
};
use super::constants::LABEL_W;
use super::style::{sonora_button, sonora_button_muted, sonora_input};
use super::widgets::{cover_thumb, fmt_duration};
use crate::core::types::TrackRow;
use crate::gui::util::{fmt_bitrate_kbps, fmt_channels, fmt_sample_rate_hz};

/// Bright teal used for mixed-field placeholder text.
const MIXED_TEAL: Color = Color::from_rgb8(0x2C, 0xE8, 0xD3);
const BUTTON_TEXT: Color = Color::from_rgb8(0xEE, 0xEE, 0xEE);
const DIRTY_TEXT: Color = Color::from_rgb8(0x33, 0xAA, 0xBB);
const SECONDARY_TEXT: Color = Color::from_rgb8(0xB8, 0xB8, 0xB8);

fn button_text<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).color(BUTTON_TEXT)
}

fn section_title<'a>(label: &'a str) -> iced::widget::Text<'a> {
    text(label).size(14)
}

fn is_mixed(state: &Sonora, field: Field) -> bool {
    state.inspector_mixed.get(&field).copied().unwrap_or(false)
}

/// Text input used by the inspector.
/// If the field is mixed:
/// - show empty value
/// - render '<mixed>' as placeholder
/// - color the placeholder bright teal
fn inspector_input<'a>(
    value: &'a str,
    mixed: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> iced::widget::TextInput<'a, Message> {
    let display_value = if mixed { "" } else { value };
    let placeholder = if mixed { mixed_display_string() } else { "" };

    text_input(placeholder, display_value)
        .on_input(on_input)
        .style(move |theme: &Theme, status: TextInputStatus| {
            let mut style = sonora_input(theme, status);
            if mixed {
                style.placeholder = MIXED_TEAL;
            }
            style
        })
}

fn field_row<'a>(
    label: &'a str,
    value: &'a str,
    mixed: bool,
    on_input: impl Fn(String) -> Message + 'a,
) -> Row<'a, Message> {
    row![
        text(label).width(Length::Fixed(LABEL_W)),
        inspector_input(value, mixed, on_input).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
}

fn num_pair_row<'a>(
    label: &'a str,
    left: &'a str,
    left_mixed: bool,
    left_on: impl Fn(String) -> Message + 'a,
    right: &'a str,
    right_mixed: bool,
    right_on: impl Fn(String) -> Message + 'a,
) -> Row<'a, Message> {
    row![
        text(label).width(Length::Fixed(LABEL_W)),
        inspector_input(left, left_mixed, left_on).width(Length::Fixed(70.0)),
        text("/"),
        inspector_input(right, right_mixed, right_on).width(Length::Fixed(70.0)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
}

fn selected_rows<'a>(state: &'a Sonora) -> Vec<&'a TrackRow> {
    let mut rows = Vec::new();

    if !state.selected_tracks.is_empty() {
        for id in &state.selected_tracks {
            if let Some(i) = state.index_of_id(*id) {
                rows.push(&state.tracks[i]);
            }
        }
    } else if let Some(id) = state.selected_track {
        if let Some(i) = state.index_of_id(id) {
            rows.push(&state.tracks[i]);
        }
    }

    rows
}

fn mixed_or_value<T: PartialEq + Copy>(
    rows: &[&TrackRow],
    getter: impl Fn(&TrackRow) -> T,
    formatter: impl Fn(T) -> String,
) -> String {
    let Some(first) = rows.first() else {
        return "―――――――――".to_string();
    };

    let first_value = getter(first);

    if rows.iter().skip(1).all(|row| getter(row) == first_value) {
        formatter(first_value)
    } else {
        mixed_display_string().to_string()
    }
}

fn build_selection_info_section(state: &Sonora) -> Column<'_, Message> {
    let rows = selected_rows(state);

    let Some(primary) = rows.first() else {
        return iced::widget::column![
            section_title("Tag Inspector"),
            text("No selection.").size(12).color(SECONDARY_TEXT),
        ]
        .spacing(8);
    };

    let sel_count = rows.len();

    let path_line = if sel_count == 1 {
        format!("{}", primary.path.display())
    } else {
        mixed_display_string().to_string()
    };

    let technical_line = format!(
        "Artwork: {} | Duration: {} | Avg. Bitrate: {} | Sample Rate: {} | Channels: {}",
        mixed_or_value(&rows, |t| t.artwork_count, |v| v.to_string()),
        mixed_or_value(&rows, |t| t.duration_ms, fmt_duration),
        mixed_or_value(&rows, |t| t.bitrate_kbps, fmt_bitrate_kbps),
        mixed_or_value(&rows, |t| t.sample_rate_hz, fmt_sample_rate_hz),
        mixed_or_value(&rows, |t| t.channels, fmt_channels),
    );

    let library_line = format!(
        "Rating: {} | Plays: {}",
        mixed_or_value(
            &rows,
            |t| t.rating,
            |v| v.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
        ),
        mixed_or_value(
            &rows,
            |t| t.play_count,
            |v| v.map(|n| n.to_string()).unwrap_or_else(|| "-".into())
        ),
    );

    // This section pulls selection metadata and technical facts into one compact
    // block so the editor fields below do not have to carry both "what is this?"
    // and "how do I edit it?" at the same time.
    iced::widget::column![
        section_title("Tag Inspector"),
        text(format!("Selected files: {sel_count}"))
            .size(12)
            .color(SECONDARY_TEXT),
        text(format!("File Path: {path_line}")).size(12),
        text(technical_line).size(12).color(SECONDARY_TEXT),
        text(library_line).size(12).color(SECONDARY_TEXT),
    ]
    .spacing(6)
}

fn artwork_preview_handle<'a>(
    state: &'a Sonora,
    rows: &[&TrackRow],
) -> Option<&'a iced::widget::image::Handle> {
    match &state.inspector_art_edit {
        ArtworkEdit::Replace { preview, .. } => Some(preview),
        ArtworkEdit::Remove => None,
        ArtworkEdit::Unchanged => {
            if rows.len() == 1 {
                state
                    .selected_track
                    .and_then(|id| state.cover_cache.get(&id))
            } else {
                let all_have_art = !rows.is_empty() && rows.iter().all(|r| r.artwork_count > 0);
                if all_have_art {
                    state
                        .selected_tracks
                        .iter()
                        .find_map(|id| state.cover_cache.get(id))
                } else {
                    None
                }
            }
        }
    }
}

fn artwork_status_text(state: &Sonora, rows: &[&TrackRow]) -> String {
    match &state.inspector_art_edit {
        ArtworkEdit::Replace { .. } => "Pending: replace artwork on save".to_string(),
        ArtworkEdit::Remove => "Pending: remove artwork on save".to_string(),
        ArtworkEdit::Unchanged => {
            if rows.is_empty() {
                "No selection.".to_string()
            } else if rows.len() == 1 {
                if rows[0].artwork_count > 0 {
                    " ".to_string()
                } else {
                    "No embedded artwork.".to_string()
                }
            } else {
                let all_have_art = rows.iter().all(|r| r.artwork_count > 0);
                let none_have_art = rows.iter().all(|r| r.artwork_count == 0);

                if all_have_art {
                    "Artwork present in all selected files.".to_string()
                } else if none_have_art {
                    "No artwork in selected files.".to_string()
                } else {
                    format!("Artwork state: {}", mixed_display_string())
                }
            }
        }
    }
}

fn build_artwork_section(state: &Sonora) -> Column<'_, Message> {
    let rows = selected_rows(state);

    let can_extract = rows.len() == 1
        && match &state.inspector_art_edit {
            ArtworkEdit::Replace { .. } => true,
            ArtworkEdit::Remove => rows[0].artwork_count > 0,
            ArtworkEdit::Unchanged => rows[0].artwork_count > 0,
        };

    let add_btn = if state.scanning || state.saving {
        button(button_text("Add / Replace artwork")).style(sonora_button)
    } else {
        button(button_text("Add / Replace artwork"))
            .on_press(Message::ChooseInspectorArtwork)
            .style(sonora_button)
    };

    let remove_btn = if state.scanning || state.saving {
        button(button_text("Remove artwork")).style(sonora_button)
    } else {
        button(button_text("Remove artwork"))
            .on_press(Message::RemoveInspectorArtwork)
            .style(sonora_button)
    };

    let extract_btn = if state.scanning || state.saving || !can_extract {
        button(button_text("Extract artwork")).style(sonora_button)
    } else {
        button(button_text("Extract artwork"))
            .on_press(Message::ExtractInspectorArtwork)
            .style(sonora_button)
    };

    // Keep artwork actions together as their own mini-workflow:
    // preview -> status -> action buttons.
    iced::widget::column![
        section_title("Artwork"),
        cover_thumb(artwork_preview_handle(state, &rows), 150.0),
        text(artwork_status_text(state, &rows))
            .size(12)
            .color(SECONDARY_TEXT),
        row![add_btn, remove_btn]
            .spacing(8)
            .align_y(Alignment::Center),
        row![extract_btn].spacing(8).align_y(Alignment::Center),
    ]
    .spacing(8)
}

fn build_primary_tag_fields(state: &Sonora) -> Column<'_, Message> {
    iced::widget::column![
        text("―――――――――").size(12).color(SECONDARY_TEXT),
        field_row(
            "Title",
            &state.inspector.title,
            is_mixed(state, Field::Title),
            |s| Message::InspectorChanged(Field::Title, s)
        ),
        field_row(
            "Artist",
            &state.inspector.artist,
            is_mixed(state, Field::Artist),
            |s| Message::InspectorChanged(Field::Artist, s)
        ),
        field_row(
            "Album",
            &state.inspector.album,
            is_mixed(state, Field::Album),
            |s| Message::InspectorChanged(Field::Album, s)
        ),
        field_row(
            "Album Artist",
            &state.inspector.album_artist,
            is_mixed(state, Field::AlbumArtist),
            |s| Message::InspectorChanged(Field::AlbumArtist, s)
        ),
        field_row(
            "Composer",
            &state.inspector.composer,
            is_mixed(state, Field::Composer),
            |s| Message::InspectorChanged(Field::Composer, s)
        ),
        num_pair_row(
            "Track",
            &state.inspector.track_no,
            is_mixed(state, Field::TrackNo),
            |s| Message::InspectorChanged(Field::TrackNo, s),
            &state.inspector.track_total,
            is_mixed(state, Field::TrackTotal),
            |s| Message::InspectorChanged(Field::TrackTotal, s),
        ),
        num_pair_row(
            "Disc",
            &state.inspector.disc_no,
            is_mixed(state, Field::DiscNo),
            |s| Message::InspectorChanged(Field::DiscNo, s),
            &state.inspector.disc_total,
            is_mixed(state, Field::DiscTotal),
            |s| Message::InspectorChanged(Field::DiscTotal, s),
        ),
        field_row(
            "Release Date",
            &state.inspector.release_date,
            is_mixed(state, Field::ReleaseDate),
            |s| Message::InspectorChanged(Field::ReleaseDate, s)
        ),
        field_row(
            "Genre",
            &state.inspector.genre,
            is_mixed(state, Field::Genre),
            |s| Message::InspectorChanged(Field::Genre, s)
        ),
        field_row(
            "Comment",
            &state.inspector.comment,
            is_mixed(state, Field::Comment),
            |s| Message::InspectorChanged(Field::Comment, s)
        ),
    ]
    .spacing(8)
}

fn build_descriptive_tag_fields(state: &Sonora) -> Column<'_, Message> {
    iced::widget::column![
        text("―――――――――").size(12).color(SECONDARY_TEXT),
        field_row(
            "Grouping",
            &state.inspector.grouping,
            is_mixed(state, Field::Grouping),
            |s| Message::InspectorChanged(Field::Grouping, s)
        ),
        field_row(
            "Subtitle",
            &state.inspector.subtitle,
            is_mixed(state, Field::Subtitle),
            |s| Message::InspectorChanged(Field::Subtitle, s)
        ),
        field_row(
            "BPM",
            &state.inspector.bpm,
            is_mixed(state, Field::Bpm),
            |s| Message::InspectorChanged(Field::Bpm, s)
        ),
        field_row(
            "Key",
            &state.inspector.key,
            is_mixed(state, Field::Key),
            |s| Message::InspectorChanged(Field::Key, s)
        ),
        field_row(
            "Mood",
            &state.inspector.mood,
            is_mixed(state, Field::Mood),
            |s| Message::InspectorChanged(Field::Mood, s)
        ),
        field_row(
            "Language",
            &state.inspector.language,
            is_mixed(state, Field::Language),
            |s| Message::InspectorChanged(Field::Language, s)
        ),
        field_row(
            "ISRC",
            &state.inspector.isrc,
            is_mixed(state, Field::Isrc),
            |s| Message::InspectorChanged(Field::Isrc, s)
        ),
    ]
    .spacing(8)
}

fn build_credit_and_tech_tag_fields(state: &Sonora) -> Column<'_, Message> {
    iced::widget::column![
        text("―――――――――").size(12).color(SECONDARY_TEXT),
        field_row(
            "Lyrics",
            &state.inspector.lyrics,
            is_mixed(state, Field::Lyrics),
            |s| Message::InspectorChanged(Field::Lyrics, s)
        ),
        field_row(
            "Lyricist",
            &state.inspector.lyricist,
            is_mixed(state, Field::Lyricist),
            |s| Message::InspectorChanged(Field::Lyricist, s)
        ),
        field_row(
            "Conductor",
            &state.inspector.conductor,
            is_mixed(state, Field::Conductor),
            |s| Message::InspectorChanged(Field::Conductor, s)
        ),
        field_row(
            "Remixer",
            &state.inspector.remixer,
            is_mixed(state, Field::Remixer),
            |s| Message::InspectorChanged(Field::Remixer, s)
        ),
        field_row(
            "Publisher",
            &state.inspector.publisher,
            is_mixed(state, Field::Publisher),
            |s| Message::InspectorChanged(Field::Publisher, s)
        ),
        field_row(
            "Encoder",
            &state.inspector.encoder_settings,
            is_mixed(state, Field::EncoderSettings),
            |s| Message::InspectorChanged(Field::EncoderSettings, s)
        ),
        field_row(
            "Encoded by",
            &state.inspector.encoded_by,
            is_mixed(state, Field::EncodedBy),
            |s| Message::InspectorChanged(Field::EncodedBy, s)
        ),
        field_row(
            "Copyright",
            &state.inspector.copyright,
            is_mixed(state, Field::Copyright),
            |s| Message::InspectorChanged(Field::Copyright, s)
        ),
    ]
    .spacing(8)
}

fn build_tags_section(state: &Sonora) -> Column<'_, Message> {
    // The inspector remains honest and thorough.
    // The improvement here is grouping, not hiding capability.
    iced::widget::column![
        section_title("Tags"),
        build_primary_tag_fields(state),
        build_descriptive_tag_fields(state),
        build_credit_and_tech_tag_fields(state),
    ]
    .spacing(12)
}

fn build_actions_section(state: &Sonora) -> Column<'_, Message> {
    // Make save feel like the main commit action in this pane.
    // The secondary button is intentionally muted so the eye knows which action
    // matters more.
    let save_btn = if state.scanning || !state.inspector_dirty {
        button(button_text("Save changes")).style(sonora_button)
    } else {
        button(button_text("Save changes"))
            .on_press(Message::SaveInspectorToFile)
            .style(sonora_button)
    };

    let close_label = if state.inspector_dirty {
        "Cancel edits"
    } else {
        "Close inspector"
    };

    let close_btn = if state.scanning {
        button(button_text(close_label)).style(sonora_button_muted)
    } else {
        button(button_text(close_label))
            .on_press(Message::CloseInspector)
            .style(sonora_button_muted)
    };

    let dirty_text = if state.inspector_dirty {
        text("Unsaved changes pending.").size(12).color(DIRTY_TEXT)
    } else {
        text("No pending changes.").size(12).color(SECONDARY_TEXT)
    };

    iced::widget::column![
        section_title("Actions"),
        dirty_text,
        row![save_btn, close_btn].spacing(8),
    ]
    .spacing(8)
}

pub(crate) fn build_inspector_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    if !state.has_selection() {
        return container(
            iced::widget::column![
                text("Tag Inspector").size(18),
                text("Select one or more tracks (center panel).").color(SECONDARY_TEXT),
            ]
            .spacing(8),
        )
        .padding(12);
    }

    let selection_info = build_selection_info_section(state);
    let artwork = build_artwork_section(state);
    let tags = build_tags_section(state);
    let actions = build_actions_section(state);

    // Scroll the content-heavy areas, but keep the action area visually separate
    // at the bottom so save/cancel does not disappear into the form wall.
    let editor = scrollable(
        iced::widget::column![selection_info, artwork, tags,]
            // Larger gap between major sections than inside them.
            .spacing(18),
    )
    .height(Length::Fill);

    container(iced::widget::column![editor, actions,].spacing(14)).padding(14)
}
