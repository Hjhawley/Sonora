//! The GUI renderer
//! This file does not mutate state
//! It reads '&Sonora' and produces widgets

use iced::Length;
use iced::widget::{Column, button, column, row, scrollable, text, text_input};
use std::collections::BTreeMap;

use super::state::{AlbumKey, LIST_HEIGHT, Message, ROOTS_HEIGHT, Sonora, ViewMode};
use super::util::{filename_stem, format_track_one_line};

pub(crate) fn view(state: &Sonora) -> Column<'_, Message> {
    // Roots UI
    let root_input = text_input("Add folder path (ex: H:\\music)", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill);

    let add_btn = if state.scanning {
        button("Add")
    } else {
        button("Add").on_press(Message::AddRootPressed)
    };

    let add_row = row![root_input, add_btn].spacing(8);

    let mut roots_list = column![];
    for (i, p) in state.roots.iter().enumerate() {
        let remove_btn = if state.scanning {
            button("×")
        } else {
            button("×").on_press(Message::RemoveRoot(i))
        };

        roots_list = roots_list.push(row![text(p.display().to_string()), remove_btn].spacing(8));
    }

    let roots_panel = scrollable(roots_list.spacing(6)).height(Length::Fixed(ROOTS_HEIGHT));

    // View mode toggle
    let albums_btn = if state.view_mode == ViewMode::Albums {
        button("Album View")
    } else {
        button("Album View").on_press(Message::SetViewMode(ViewMode::Albums))
    };

    let tracks_btn = if state.view_mode == ViewMode::Tracks {
        button("Track View")
    } else {
        button("Track View").on_press(Message::SetViewMode(ViewMode::Tracks))
    };

    let view_toggle = row![albums_btn, tracks_btn].spacing(8);

    // Scan button
    let scan_btn = if state.scanning {
        button("Scanning…")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    // Main list (Albums or Tracks)
    let main_list = match state.view_mode {
        ViewMode::Tracks => build_tracks_list(state),
        ViewMode::Albums => build_albums_list(state),
    };

    // Inspector panel (right side)
    let inspector_panel = build_inspector(state);

    let body = row![
        column![scan_btn, main_list]
            .spacing(10)
            .width(Length::FillPortion(2)),
        inspector_panel.width(Length::FillPortion(1)),
    ]
    .spacing(12);

    column![
        text("Sonora"),
        text(&state.status),
        add_row,
        roots_panel,
        view_toggle,
        body,
    ]
    .spacing(12)
}

fn build_tracks_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let mut list = column![];

    for (i, t) in state.tracks.iter().enumerate() {
        let label = format_track_one_line(t);

        let prefix = if state.selected_track == Some(i) {
            "▶ "
        } else {
            "  "
        };

        list =
            list.push(button(text(format!("{prefix}{label}"))).on_press(Message::SelectTrack(i)));
    }

    scrollable(list.spacing(6)).height(Length::Fixed(LIST_HEIGHT))
}

fn build_albums_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let mut groups: BTreeMap<AlbumKey, Vec<usize>> = BTreeMap::new();

    for (i, t) in state.tracks.iter().enumerate() {
        // Prefer Album Artist for album grouping
        // Fallback chain: album_artist -> artist -> "Unknown Album Artist"
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown Album Artist".to_string());

        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        let key = AlbumKey {
            artist: album_artist,
            album,
        };

        groups.entry(key).or_default().push(i);
    }

    let mut list = column![];

    for (key, track_indexes) in groups {
        let is_selected_album = state.selected_album.as_ref() == Some(&key);

        let album_label = format!(
            "{} — {} ({} tracks)",
            key.artist,
            key.album,
            track_indexes.len()
        );

        let album_prefix = if is_selected_album { "[-] " } else { "[+] " };

        list = list.push(
            button(text(format!("{album_prefix}{album_label}")))
                .on_press(Message::SelectAlbum(key.clone())),
        );

        if is_selected_album {
            for i in track_indexes {
                let t = &state.tracks[i];

                let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
                let track_no = t
                    .track_no
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "??".to_string());

                let track_line = format!("    #{track_no} — {title}");

                let prefix = if state.selected_track == Some(i) {
                    "    ▶ "
                } else {
                    "      "
                };

                list = list.push(
                    button(text(format!("{prefix}{track_line}"))).on_press(Message::SelectTrack(i)),
                );
            }
        }
    }

    scrollable(list.spacing(6)).height(Length::Fixed(LIST_HEIGHT))
}

fn build_inspector(state: &Sonora) -> Column<'_, Message> {
    let Some(i) = state.selected_track else {
        return column![
            text("Metadata inspector"),
            text("Select a track to edit metadata."),
            text("(Edits are not actually written to files for now.)"),
        ]
        .spacing(8);
    };

    if i >= state.tracks.len() {
        return column![
            text("Metadata inspector"),
            text("Invalid selection, rescan?")
        ]
        .spacing(8);
    }

    let t = &state.tracks[i];

    let path_line = format!("Path:\n{}", t.path.display());

    let title = text_input("Title", &state.inspector.title)
        .on_input(Message::EditTitle)
        .width(Length::Fill);

    let artist = text_input("Artist", &state.inspector.artist)
        .on_input(Message::EditArtist)
        .width(Length::Fill);

    let album = text_input("Album", &state.inspector.album)
        .on_input(Message::EditAlbum)
        .width(Length::Fill);

    let track_no = text_input("Track #", &state.inspector.track_no)
        .on_input(Message::EditTrackNo)
        .width(Length::Fill);

    let year = text_input("Year", &state.inspector.year)
        .on_input(Message::EditYear)
        .width(Length::Fill);

    let save_btn = if state.scanning || !state.inspector_dirty {
        button("Save edits")
    } else {
        button("Save edits").on_press(Message::SaveInspectorToMemory)
    };

    let revert_btn = if state.scanning {
        button("Cancel edits")
    } else {
        button("Cancel edits").on_press(Message::RevertInspector)
    };

    column![
        text("Metadata inspector"),
        text(path_line),
        title,
        artist,
        album,
        row![track_no, year].spacing(8),
        row![save_btn, revert_btn].spacing(8),
    ]
    .spacing(10)
}
