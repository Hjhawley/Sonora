//! Sonora GUI
//!
//! Current behavior (read-only)
//! - User adds one or more folder roots.
//! - "Scan Library" walks roots for `.mp3`, reads ID3 into `TrackRow`.
//! - Library is displayed as either:
//!   - Track View: flat list of tracks
//!   - Album View: grouped by (artist, album), with expandable albums
//! - Selecting a track populates the Inspector form.
//! - "Save edits" updates the in-memory `TrackRow` only (NO disk writes).
//!
//! Not implemented yet
//! - Writing tags back to files
//! - Persistent DB/cache
//! - Audio playback
//!
//! Architecture constraints (on purpose)
//! - UI layer calls `core::*` for scanning/tag reading.
//! - UI does not perform filesystem IO except validating user-entered root paths.
//!
//! Concurrency model
//! - Scan runs on a spawned thread; UI stays responsive.
//! - Result is returned via a oneshot channel back into the Iced update loop.

mod core;

use iced::widget::{Column, button, column, row, scrollable, text, text_input};
use iced::{Length, Task};

use iced::futures::channel::oneshot;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::types::TrackRow;

// If user hasn’t added any folders yet, scan ./test so you can iterate fast
const TEST_ROOT: &str = "test";

const ROOTS_HEIGHT: f32 = 120.0;
const LIST_HEIGHT: f32 = 460.0;

// View mode: Albums vs Tracks

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Albums,
    Tracks,
}

// We identify an album by (album title + artist).
// Later you’ll likely use Album Artist or a real AlbumId.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AlbumKey {
    artist: String,
    album: String,
}

// Inspector = the editable UI state

// IMPORTANT: this is NOT writing to disk yet.
// It’s just the text boxes on the right.
#[derive(Debug, Default, Clone)]
struct InspectorDraft {
    title: String,
    artist: String,
    album: String,
    track_no: String,
    year: String,
}

struct Sonora {
    status: String,
    scanning: bool,

    // Folder roots UI
    root_input: String,
    roots: Vec<PathBuf>,

    // Loaded tracks
    tracks: Vec<TrackRow>,

    // New UI structure
    view_mode: ViewMode,
    selected_album: Option<AlbumKey>, // used in Albums view
    selected_track: Option<usize>,    // index into tracks

    // Right-side inspector "form"
    inspector: InspectorDraft,
    inspector_dirty: bool, // true when user typed something
}

impl Default for Sonora {
    fn default() -> Self {
        Self {
            status: "Add a folder, then Scan.".to_string(),
            scanning: false,

            root_input: String::new(),
            roots: Vec::new(),

            tracks: Vec::new(),

            view_mode: ViewMode::Tracks,
            selected_album: None,
            selected_track: None,

            inspector: InspectorDraft::default(),
            inspector_dirty: false,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    // Roots UI
    RootInputChanged(String),
    AddRootPressed,
    RemoveRoot(usize),

    // Scan
    ScanLibrary,
    ScanFinished(Result<(Vec<TrackRow>, usize), String>),

    // View mode + selection
    SetViewMode(ViewMode),
    SelectAlbum(AlbumKey),
    SelectTrack(usize),

    // Inspector editing
    EditTitle(String),
    EditArtist(String),
    EditAlbum(String),
    EditTrackNo(String),
    EditYear(String),

    SaveInspectorToMemory,
    RevertInspector,
}

fn main() -> iced::Result {
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .run()
}

fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        // Roots (folders)
        Message::RootInputChanged(s) => {
            state.root_input = s;
            Task::none()
        }

        Message::AddRootPressed => {
            let input = state.root_input.trim();
            if input.is_empty() {
                return Task::none();
            }

            let p = PathBuf::from(input);

            // Don’t let user add garbage paths
            if !Path::new(input).is_dir() {
                state.status = format!("Not a folder: {}", p.display());
                return Task::none();
            }

            // Don’t add duplicates
            if state.roots.contains(&p) {
                state.status = format!("Already added: {}", p.display());
                state.root_input.clear();
                return Task::none();
            }

            state.roots.push(p.clone());
            state.root_input.clear();
            state.status = format!("Added folder: {}", p.display());

            Task::none()
        }

        Message::RemoveRoot(i) => {
            if i < state.roots.len() && !state.scanning {
                let removed = state.roots.remove(i);
                state.status = format!("Removed folder: {}", removed.display());
            }
            Task::none()
        }

        // Scan
        Message::ScanLibrary => {
            if state.scanning {
                return Task::none();
            }

            state.scanning = true;
            state.tracks.clear();
            state.status = "Scanning…".to_string();

            // If user hasn’t added roots, scan ./test
            let roots_to_scan = if state.roots.is_empty() {
                vec![PathBuf::from(TEST_ROOT)]
            } else {
                state.roots.clone()
            };

            Task::perform(
                async move {
                    let (tx, rx) = oneshot::channel::<Result<(Vec<TrackRow>, usize), String>>();

                    std::thread::spawn(move || {
                        // This is the "heavy work" thread
                        let _ = tx.send(crate::core::scan_and_read_roots(roots_to_scan));
                    });

                    rx.await
                        .map_err(|_| "Scan thread dropped without returning".to_string())?
                },
                Message::ScanFinished,
            )
        }

        Message::ScanFinished(result) => {
            state.scanning = false;

            match result {
                Ok((mut rows, tag_failures)) => {
                    rows.sort_by(|a, b| a.path.cmp(&b.path));

                    state.status = if tag_failures == 0 {
                        format!("Loaded {} tracks", rows.len())
                    } else {
                        format!(
                            "Loaded {} tracks ({} tag read failures)",
                            rows.len(),
                            tag_failures
                        )
                    };

                    state.tracks = rows;

                    // After rescanning, your selections may not make sense anymore.
                    // Keep it simple: clear selection for now.
                    state.selected_track = None;
                    state.selected_album = None;
                    clear_inspector(state);
                }
                Err(e) => {
                    state.status = format!("Error: {e}");
                    state.tracks.clear();
                }
            }

            Task::none()
        }

        // View mode
        Message::SetViewMode(mode) => {
            state.view_mode = mode;

            // Switching views should feel predictable:
            // clear selection (you can change this later if you want).
            state.selected_track = None;
            clear_inspector(state);

            if mode == ViewMode::Tracks {
                state.selected_album = None;
            }

            Task::none()
        }

        // Album selection
        Message::SelectAlbum(key) => {
            state.selected_album = Some(key);
            state.selected_track = None; // selecting an album is not selecting a track
            clear_inspector(state);
            Task::none()
        }

        // Track selection
        Message::SelectTrack(i) => {
            if i < state.tracks.len() {
                state.selected_track = Some(i);
                load_inspector_from_track(state);
            }
            Task::none()
        }

        // Inspector typing
        Message::EditTitle(s) => {
            state.inspector.title = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditArtist(s) => {
            state.inspector.artist = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditAlbum(s) => {
            state.inspector.album = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditTrackNo(s) => {
            state.inspector.track_no = s;
            state.inspector_dirty = true;
            Task::none()
        }
        Message::EditYear(s) => {
            state.inspector.year = s;
            state.inspector_dirty = true;
            Task::none()
        }

        // Save = ONLY updates memory (NOT disk yet)
        Message::SaveInspectorToMemory => {
            let Some(i) = state.selected_track else {
                state.status = "Select a track first.".to_string();
                return Task::none();
            };

            if i >= state.tracks.len() {
                return Task::none();
            }

            // Parse numbers (if user typed garbage, show an error)
            let track_no = parse_optional_u32(&state.inspector.track_no)
                .map_err(|_| "Track # must be a number".to_string());

            let year = parse_optional_i32(&state.inspector.year)
                .map_err(|_| "Year must be a number".to_string());

            // Borrow the errors without moving the Results
            let track_err = track_no.as_ref().err();
            let year_err = year.as_ref().err();

            if track_err.is_some() || year_err.is_some() {
                let mut msg = String::from("Not saved: ");

                if let Some(e) = track_err {
                    msg.push_str(e);
                }

                if track_err.is_some() && year_err.is_some() {
                    msg.push_str(" | ");
                }

                if let Some(e) = year_err {
                    msg.push_str(e);
                }

                state.status = msg;
                return Task::none();
            }

            // Now it's safe to unwrap because we know they're Ok(...)
            let track_no = track_no.unwrap();
            let year = year.unwrap();

            // Write values into the in-memory TrackRow
            let t = &mut state.tracks[i];

            t.title = clean_optional_string(&state.inspector.title);
            t.artist = clean_optional_string(&state.inspector.artist);
            t.album = clean_optional_string(&state.inspector.album);
            t.track_no = track_no;
            t.year = year;

            state.inspector_dirty = false;
            state.status = "Changes saved to memory, not written to files (yet)".to_string();

            Task::none()
        }

        Message::RevertInspector => {
            // Put the inspector back to whatever the track currently says
            load_inspector_from_track(state);
            Task::none()
        }
    }
}

// View (UI)

fn view(state: &Sonora) -> Column<'_, Message> {
    // ------- Roots UI -------
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

    // ------- View mode toggle -------
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

    // ------- Scan button -------
    let scan_btn = if state.scanning {
        button("Scanning…")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    // ------- Main list (Albums or Tracks) -------
    let main_list = match state.view_mode {
        ViewMode::Tracks => build_tracks_list(state),
        ViewMode::Albums => build_albums_list(state),
    };

    // ------- Inspector panel (right side) -------
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

// Build Tracks list (click to select a track)

fn build_tracks_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let mut list = column![];

    // Optional: if Albums view selected one album, you’ll later filter here.
    // For now Tracks view just shows everything.

    for (i, t) in state.tracks.iter().enumerate() {
        let label = format_track_one_line(t);

        // Show a simple marker for the currently selected track
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

// Build Albums list (grouped)
// Click an album = expands its tracks

fn build_albums_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    // Group tracks by (artist, album)
    let mut groups: BTreeMap<AlbumKey, Vec<usize>> = BTreeMap::new();

    for (i, t) in state.tracks.iter().enumerate() {
        let artist = t
            .artist
            .clone()
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album = t
            .album
            .clone()
            .unwrap_or_else(|| "Unknown Album".to_string());

        let key = AlbumKey { artist, album };
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

        let album_prefix = if is_selected_album { "▼ " } else { "▶ " };

        list = list.push(
            button(text(format!("{album_prefix}{album_label}")))
                .on_press(Message::SelectAlbum(key.clone())),
        );

        // If this album is selected, show its tracks underneath (indented)
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

// Inspector UI (right panel)

fn build_inspector(state: &Sonora) -> Column<'_, Message> {
    // If nothing is selected, show an empty state
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

    // Disable save if scanning or nothing changed
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
        text("Inspector"),
        text(path_line),
        title,
        artist,
        album,
        row![track_no, year].spacing(8),
        row![save_btn, revert_btn].spacing(8),
    ]
    .spacing(10)
}

// Helpers

fn load_inspector_from_track(state: &mut Sonora) {
    let Some(i) = state.selected_track else {
        clear_inspector(state);
        return;
    };

    if i >= state.tracks.len() {
        clear_inspector(state);
        return;
    }

    let t = &state.tracks[i];

    state.inspector.title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    state.inspector.artist = t
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown Artist".to_string());
    state.inspector.album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    state.inspector.track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.year = t.year.map(|y| y.to_string()).unwrap_or_default();

    state.inspector_dirty = false;
}

fn clear_inspector(state: &mut Sonora) {
    state.inspector = InspectorDraft::default();
    state.inspector_dirty = false;
}

fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Title")
        .to_string()
}

// A compact one-line view for Track rows (good for tables/lists)
fn format_track_one_line(t: &TrackRow) -> String {
    let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    let artist = t
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    let track_no = t
        .track_no
        .map(|n| n.to_string())
        .unwrap_or_else(|| "??".to_string());

    format!("#{track_no} — {artist} — {title} ({album})")
}

fn clean_optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_optional_u32(s: &str) -> Result<Option<u32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<u32>().map(Some).map_err(|_| ())
}

fn parse_optional_i32(s: &str) -> Result<Option<i32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| ())
}
