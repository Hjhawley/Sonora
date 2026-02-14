//! GUI renderer (reads state, produces widgets; no mutation).

use iced::Length;
use iced::widget::{Column, button, checkbox, column, row, scrollable, text, text_input};
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

    // Inspector panel
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
        // IMPORTANT: group by ALBUM ARTIST, not track artist
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        let key = AlbumKey {
            album_artist,
            album,
        };
        groups.entry(key).or_default().push(i);
    }

    let mut list = column![];

    for (key, track_indexes) in groups {
        let is_selected_album = state.selected_album.as_ref() == Some(&key);

        let album_label = format!(
            "{} — {} ({} tracks)",
            key.album_artist,
            key.album,
            track_indexes.len()
        );

        let album_prefix = if is_selected_album { "[-] " } else { "[+] " };

        // Click toggles expand/collapse (update.rs handles it)
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

    // Core inputs
    let title = text_input("Title", &state.inspector.title)
        .on_input(Message::EditTitle)
        .width(Length::Fill);
    let artist = text_input("Artist (track)", &state.inspector.artist)
        .on_input(Message::EditArtist)
        .width(Length::Fill);
    let album_artist = text_input("Album Artist", &state.inspector.album_artist)
        .on_input(Message::EditAlbumArtist)
        .width(Length::Fill);
    let album = text_input("Album", &state.inspector.album)
        .on_input(Message::EditAlbum)
        .width(Length::Fill);
    let composer = text_input("Composer", &state.inspector.composer)
        .on_input(Message::EditComposer)
        .width(Length::Fill);

    let track_no = text_input("Track #", &state.inspector.track_no)
        .on_input(Message::EditTrackNo)
        .width(Length::Fill);
    let track_total = text_input("Track total", &state.inspector.track_total)
        .on_input(Message::EditTrackTotal)
        .width(Length::Fill);
    let disc_no = text_input("Disc #", &state.inspector.disc_no)
        .on_input(Message::EditDiscNo)
        .width(Length::Fill);
    let disc_total = text_input("Disc total", &state.inspector.disc_total)
        .on_input(Message::EditDiscTotal)
        .width(Length::Fill);

    let year = text_input("Year", &state.inspector.year)
        .on_input(Message::EditYear)
        .width(Length::Fill);
    let date = text_input("Date (TDRC)", &state.inspector.date)
        .on_input(Message::EditDate)
        .width(Length::Fill);
    let genre = text_input("Genre", &state.inspector.genre)
        .on_input(Message::EditGenre)
        .width(Length::Fill);

    // Toggle for extended tags
    let extended_toggle = checkbox(state.show_extended)
        .label("Show extended tags")
        .on_toggle(Message::ToggleExtended);

    // Buttons
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

    // Read-only stats (also kills dead_code warnings)
    let stats_line = format!(
        "Artwork: {} | Duration(ms): {} | Rating: {} | Plays: {} | Compilation: {}",
        t.artwork_count,
        t.duration_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        t.rating
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        t.play_count
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
        t.compilation
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into()),
    );

    let maps_line = format!(
        "User text: {} | URLs: {} | Extra text: {}",
        t.user_text.len(),
        t.urls.len(),
        t.extra_text.len(),
    );

    let mut col = column![
        text("Metadata inspector"),
        text(path_line),
        text(stats_line),
        text(maps_line),
        title,
        row![artist, album_artist].spacing(8),
        row![album, composer].spacing(8),
        row![track_no, track_total].spacing(8),
        row![disc_no, disc_total].spacing(8),
        row![year, genre].spacing(8),
        date,
        extended_toggle,
    ]
    .spacing(10);

    if state.show_extended {
        let lyricist = text_input("Lyricist", &state.inspector.lyricist)
            .on_input(Message::EditLyricist)
            .width(Length::Fill);
        let conductor = text_input("Conductor", &state.inspector.conductor)
            .on_input(Message::EditConductor)
            .width(Length::Fill);
        let remixer = text_input("Remixer", &state.inspector.remixer)
            .on_input(Message::EditRemixer)
            .width(Length::Fill);
        let publisher = text_input("Publisher", &state.inspector.publisher)
            .on_input(Message::EditPublisher)
            .width(Length::Fill);
        let grouping = text_input("Grouping", &state.inspector.grouping)
            .on_input(Message::EditGrouping)
            .width(Length::Fill);
        let subtitle = text_input("Subtitle", &state.inspector.subtitle)
            .on_input(Message::EditSubtitle)
            .width(Length::Fill);
        let bpm = text_input("BPM", &state.inspector.bpm)
            .on_input(Message::EditBpm)
            .width(Length::Fill);
        let key = text_input("Key", &state.inspector.key)
            .on_input(Message::EditKey)
            .width(Length::Fill);
        let mood = text_input("Mood", &state.inspector.mood)
            .on_input(Message::EditMood)
            .width(Length::Fill);
        let language = text_input("Language", &state.inspector.language)
            .on_input(Message::EditLanguage)
            .width(Length::Fill);
        let isrc = text_input("ISRC", &state.inspector.isrc)
            .on_input(Message::EditIsrc)
            .width(Length::Fill);
        let encoder_settings = text_input("Encoder settings", &state.inspector.encoder_settings)
            .on_input(Message::EditEncoderSettings)
            .width(Length::Fill);
        let encoded_by = text_input("Encoded by", &state.inspector.encoded_by)
            .on_input(Message::EditEncodedBy)
            .width(Length::Fill);
        let copyright = text_input("Copyright", &state.inspector.copyright)
            .on_input(Message::EditCopyright)
            .width(Length::Fill);
        let comment = text_input("Comment", &state.inspector.comment)
            .on_input(Message::EditComment)
            .width(Length::Fill);
        let lyrics = text_input("Lyrics (USLT)", &state.inspector.lyrics)
            .on_input(Message::EditLyrics)
            .width(Length::Fill);

        col = col
            .push(lyricist)
            .push(row![conductor, remixer].spacing(8))
            .push(row![publisher, grouping].spacing(8))
            .push(subtitle)
            .push(row![bpm, key].spacing(8))
            .push(row![mood, language].spacing(8))
            .push(isrc)
            .push(encoder_settings)
            .push(encoded_by)
            .push(copyright)
            .push(comment)
            .push(lyrics);
    }

    col.push(row![save_btn, revert_btn].spacing(8))
}
