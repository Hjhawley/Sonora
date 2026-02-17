//! GUI renderer (reads state, produces widgets; no mutation).

use iced::widget::{
    Column, button, checkbox, column, container, mouse_area, row, scrollable, text, text_input,
};
use iced::{Alignment, Length};
use std::collections::BTreeMap;

use super::state::{AlbumKey, Message, Sonora, ViewMode};
use super::util::filename_stem;

const PLAYBACK_H: f32 = 76.0;

const SIDEBAR_W: f32 = 260.0;
const EDITOR_W: f32 = 380.0;

const LABEL_W: f32 = 110.0;

// list sizing
const HEADER_TEXT: f32 = 14.0;
const ROW_TEXT: f32 = 14.0;

const TRACK_ROW_H: f32 = 26.0;
const TRACK_ROW_VPAD: f32 = 2.0;
const TRACK_ROW_HPAD: f32 = 8.0;
const TRACK_LIST_SPACING: f32 = 1.0;

const ALBUM_LIST_H: f32 = 260.0;
const ALBUM_ROW_H: f32 = 56.0;
const ALBUM_ROW_COVER: f32 = 44.0;
const ALBUM_LIST_SPACING: f32 = 1.0;

const COVER_BIG: f32 = 220.0;

fn fmt_duration(ms: Option<u32>) -> String {
    let Some(ms) = ms else { return "-".into() };
    let s = ms / 1000;
    let m = s / 60;
    let s = s % 60;
    format!("{m}:{s:02}")
}

fn cover_placeholder(size: f32) -> iced::widget::Container<'static, Message> {
    container(
        column![text("♪").size(28), text("cover").size(12)]
            .spacing(4)
            .align_x(Alignment::Center),
    )
    .width(Length::Fixed(size))
    .height(Length::Fixed(size))
    .center_x(Length::Fill)
    .center_y(Length::Fill)
}

fn field_row<'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> iced::widget::Row<'a, Message> {
    row![
        text(label).width(Length::Fixed(LABEL_W)),
        text_input("", value).on_input(on_input).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
}

fn num_pair_row<'a>(
    label: &'a str,
    left: &'a str,
    left_on: impl Fn(String) -> Message + 'a,
    right: &'a str,
    right_on: impl Fn(String) -> Message + 'a,
) -> iced::widget::Row<'a, Message> {
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

pub(crate) fn view(state: &Sonora) -> Column<'_, Message> {
    let playback = build_playback_bar().height(Length::Fixed(PLAYBACK_H));

    let sidebar = build_sidebar(state).width(Length::Fixed(SIDEBAR_W));
    let main = build_center_panel(state).width(Length::Fill);
    let editor = build_inspector_panel(state).width(Length::Fixed(EDITOR_W));

    let body = row![sidebar, main, editor].spacing(12).height(Length::Fill);

    column![playback, body].spacing(12).padding(12)
}

fn build_playback_bar() -> iced::widget::Container<'static, Message> {
    container(row![text("playback (not yet implemented)").size(28)].align_y(Alignment::Center))
        .padding(16)
}

fn build_sidebar(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let scan_btn = if state.scanning {
        button("Scanning...")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    // Make the active mode visually obvious (✓) instead of relying on “disabled button look”.
    let albums_btn = if state.view_mode == ViewMode::Albums {
        button("✓ Album View")
    } else {
        button("Album View").on_press(Message::SetViewMode(ViewMode::Albums))
    };

    let tracks_btn = if state.view_mode == ViewMode::Tracks {
        button("✓ Track View")
    } else {
        button("Track View").on_press(Message::SetViewMode(ViewMode::Tracks))
    };

    let view_toggle = row![albums_btn, tracks_btn].spacing(8);

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
    let roots_panel = scrollable(roots_list.spacing(6)).height(Length::Fixed(160.0));

    let playlists = column![
        text("Playlists").size(16),
        button("Library"),
        button("Favorites (coming soon)"),
        button("Recently added (coming soon)"),
    ]
    .spacing(6);

    let col = column![
        text("Sonora").size(20),
        text(&state.status).size(12),
        scan_btn,
        view_toggle,
        text("Library folders").size(16),
        add_row,
        roots_panel,
        playlists,
    ]
    .spacing(12);

    container(scrollable(col).height(Length::Fill)).padding(12)
}

fn build_center_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
    let inner: iced::Element<'_, Message> = match state.view_mode {
        ViewMode::Tracks => build_tracks_center(state).into(),
        ViewMode::Albums => build_albums_center(state).into(),
    };

    container(inner).padding(12)
}

// Track view

fn build_tracks_center(state: &Sonora) -> Column<'_, Message> {
    column![
        text("Tracks").size(18),
        build_tracks_table(state).height(Length::Fill),
    ]
    .spacing(12)
}

fn build_tracks_table(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let header = row![
        text("").size(HEADER_TEXT).width(Length::Fixed(24.0)),
        text("#").size(HEADER_TEXT).width(Length::Fixed(44.0)),
        text("Title").size(HEADER_TEXT).width(Length::Fixed(240.0)),
        text("Artist").size(HEADER_TEXT).width(Length::Fixed(190.0)),
        text("Album").size(HEADER_TEXT).width(Length::Fixed(240.0)),
        text("Album Artist")
            .size(HEADER_TEXT)
            .width(Length::Fixed(170.0)),
        text("Year").size(HEADER_TEXT).width(Length::Fixed(70.0)),
        text("Genre").size(HEADER_TEXT).width(Length::Fixed(140.0)),
        text("Len").size(HEADER_TEXT).width(Length::Fixed(70.0)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut col = column![header].spacing(TRACK_LIST_SPACING);

    for (i, t) in state.tracks.iter().enumerate() {
        let selected = state.selected_track == Some(i);
        let marker = if selected { "▶" } else { "" };

        let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let album = t.album.clone().unwrap_or_else(|| "Unknown".into());
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown".into());
        let year = t.year.map(|y| y.to_string()).unwrap_or_default();
        let genre = t.genre.clone().unwrap_or_default();
        let len = fmt_duration(t.duration_ms);

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(24.0)),
            text(track_no).size(ROW_TEXT).width(Length::Fixed(44.0)),
            text(title).size(ROW_TEXT).width(Length::Fixed(240.0)),
            text(artist).size(ROW_TEXT).width(Length::Fixed(190.0)),
            text(album).size(ROW_TEXT).width(Length::Fixed(240.0)),
            text(album_artist)
                .size(ROW_TEXT)
                .width(Length::Fixed(170.0)),
            text(year).size(ROW_TEXT).width(Length::Fixed(70.0)),
            text(genre).size(ROW_TEXT).width(Length::Fixed(140.0)),
            text(len).size(ROW_TEXT).width(Length::Fixed(70.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // IMPORTANT: mouse_area gives you “clickable row” without button chrome.
        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectTrack(i));

        col = col.push(row_widget);
    }

    scrollable(col).height(Length::Fill)
}

// Album view

fn build_albums_center(state: &Sonora) -> Column<'_, Message> {
    // group tracks -> albums (LOCAL)
    let mut groups: BTreeMap<AlbumKey, Vec<usize>> = BTreeMap::new();

    for (i, t) in state.tracks.iter().enumerate() {
        let album_artist = t
            .album_artist
            .clone()
            .or_else(|| t.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        groups
            .entry(AlbumKey {
                album_artist,
                album,
            })
            .or_default()
            .push(i);
    }

    // Build owned rows (no borrowing issues)
    let selected_key: Option<AlbumKey> = state.selected_album.clone();
    let albums: Vec<(AlbumKey, usize)> = groups.iter().map(|(k, v)| (k.clone(), v.len())).collect();

    let list = build_album_list(selected_key.clone(), albums);

    let selected_payload: Option<(AlbumKey, Vec<usize>)> = state
        .selected_album
        .as_ref()
        .and_then(|k| groups.get(k).map(|v| (k.clone(), v.clone())));

    let detail = build_album_detail(state, selected_payload);

    column![
        text("Albums").size(18),
        list.height(Length::Fixed(ALBUM_LIST_H)),
        detail.height(Length::Fill),
    ]
    .spacing(12)
}

fn build_album_list(
    selected: Option<AlbumKey>,
    albums: Vec<(AlbumKey, usize)>,
) -> iced::widget::Scrollable<'static, Message> {
    let mut col: Column<'static, Message> = column![].spacing(ALBUM_LIST_SPACING);

    for (key, count) in albums {
        let is_selected = selected.as_ref() == Some(&key);
        let marker = if is_selected { "●" } else { "" };

        let title_line = format!("{marker} {}", key.album);
        let artist_line = key.album_artist.clone();
        let count_line = format!("{count} tracks");

        let row_cells = row![
            cover_placeholder(ALBUM_ROW_COVER),
            column![text(title_line).size(14), text(artist_line).size(12),]
                .spacing(2)
                .width(Length::Fill),
            text(count_line).size(12).width(Length::Fixed(90.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([6, 8])
                .height(Length::Fixed(ALBUM_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectAlbum(key));

        col = col.push(row_widget);
    }

    scrollable(col)
}

fn build_album_detail(
    state: &Sonora,
    selected: Option<(AlbumKey, Vec<usize>)>,
) -> iced::widget::Container<'_, Message> {
    let Some((key, track_idxs)) = selected else {
        return container(text("Select an album to view tracks.")).padding(12);
    };

    if track_idxs.is_empty() {
        return container(text("Album has no tracks (weird).")).padding(12);
    }

    let mut idxs = track_idxs;
    idxs.sort_by(|&a, &b| {
        let ta = &state.tracks[a];
        let tb = &state.tracks[b];
        (
            ta.disc_no.unwrap_or(0),
            ta.track_no.unwrap_or(0),
            ta.title.clone().unwrap_or_default(),
        )
            .cmp(&(
                tb.disc_no.unwrap_or(0),
                tb.track_no.unwrap_or(0),
                tb.title.clone().unwrap_or_default(),
            ))
    });

    let first = &state.tracks[idxs[0]];
    let year = first
        .year
        .map(|y| y.to_string())
        .unwrap_or_else(|| "-".into());
    let genre = first.genre.clone().unwrap_or_else(|| "-".into());

    let header = row![
        cover_placeholder(COVER_BIG),
        column![
            text(key.album.clone()).size(26),
            text(key.album_artist.clone()).size(18),
            text(format!("{genre} • {year}")).size(14),
            text(format!("{} songs", idxs.len())).size(12),
        ]
        .spacing(6)
        .width(Length::Fill),
    ]
    .spacing(18)
    .align_y(Alignment::Center);

    let mut list = column![].spacing(TRACK_LIST_SPACING);

    for &i in &idxs {
        let t = &state.tracks[i];
        let n = t
            .track_no
            .map(|n| n.to_string())
            .unwrap_or_else(|| "—".into());
        let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown".into());
        let dur = fmt_duration(t.duration_ms);

        let selected = state.selected_track == Some(i);
        let marker = if selected { "▶" } else { "" };

        let row_cells = row![
            text(marker).size(ROW_TEXT).width(Length::Fixed(24.0)),
            text(n).size(ROW_TEXT).width(Length::Fixed(32.0)),
            column![text(title).size(ROW_TEXT), text(artist).size(12)]
                .spacing(2)
                .width(Length::Fill),
            text(dur).size(ROW_TEXT).width(Length::Fixed(60.0)),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let row_widget = mouse_area(
            container(row_cells)
                .padding([TRACK_ROW_VPAD, TRACK_ROW_HPAD])
                .height(Length::Fixed(TRACK_ROW_H))
                .width(Length::Fill),
        )
        .on_press(Message::SelectTrack(i));

        list = list.push(row_widget);
    }

    let tracks_panel = scrollable(list).height(Length::Fill);

    container(column![header, tracks_panel].spacing(12)).padding(12)
}

// Metadata inspector

fn build_inspector_panel(state: &Sonora) -> iced::widget::Container<'_, Message> {
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

    let core = column![
        field_row("Title", &state.inspector.title, Message::EditTitle),
        field_row("Artist", &state.inspector.artist, Message::EditArtist),
        field_row("Album", &state.inspector.album, Message::EditAlbum),
        field_row(
            "Album Artist",
            &state.inspector.album_artist,
            Message::EditAlbumArtist
        ),
        field_row("Composer", &state.inspector.composer, Message::EditComposer),
        num_pair_row(
            "Track",
            &state.inspector.track_no,
            Message::EditTrackNo,
            &state.inspector.track_total,
            Message::EditTrackTotal,
        ),
        num_pair_row(
            "Disc",
            &state.inspector.disc_no,
            Message::EditDiscNo,
            &state.inspector.disc_total,
            Message::EditDiscTotal,
        ),
        field_row("Year", &state.inspector.year, Message::EditYear),
        field_row("Genre", &state.inspector.genre, Message::EditGenre),
        field_row("Date", &state.inspector.date, Message::EditDate),
    ]
    .spacing(8);

    let toggle = checkbox(state.show_extended)
        .label("Show extended tags")
        .on_toggle(Message::ToggleExtended);

    let mut extended = column![];
    if state.show_extended {
        extended = column![
            field_row("Grouping", &state.inspector.grouping, Message::EditGrouping),
            field_row("Comment", &state.inspector.comment, Message::EditComment),
            field_row("Lyrics", &state.inspector.lyrics, Message::EditLyrics),
            field_row("Lyricist", &state.inspector.lyricist, Message::EditLyricist),
            field_row(
                "Conductor",
                &state.inspector.conductor,
                Message::EditConductor
            ),
            field_row("Remixer", &state.inspector.remixer, Message::EditRemixer),
            field_row(
                "Publisher",
                &state.inspector.publisher,
                Message::EditPublisher
            ),
            field_row("Subtitle", &state.inspector.subtitle, Message::EditSubtitle),
            field_row("BPM", &state.inspector.bpm, Message::EditBpm),
            field_row("Key", &state.inspector.key, Message::EditKey),
            field_row("Mood", &state.inspector.mood, Message::EditMood),
            field_row("Language", &state.inspector.language, Message::EditLanguage),
            field_row("ISRC", &state.inspector.isrc, Message::EditIsrc),
            field_row(
                "Encoder",
                &state.inspector.encoder_settings,
                Message::EditEncoderSettings
            ),
            field_row(
                "Encoded by",
                &state.inspector.encoded_by,
                Message::EditEncodedBy
            ),
            field_row(
                "Copyright",
                &state.inspector.copyright,
                Message::EditCopyright
            ),
        ]
        .spacing(8);
    }

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
