//! Sonora GUI
//!
//! # What this program is
//! A small desktop app (built with the `iced` GUI library) that scans folders for `.mp3` files,
//! reads ID3 tags (title/artist/album/etc), and shows them in a UI.
//!
//! # How Iced works (super simple mental model)
//! Think “video game loop”, but message-based:
//!
//! - `Sonora` = the *entire memory* of the app (all the state)
//! - `Message` = “something happened” (button clicked, typed a letter, scan finished)
//! - `update(state, message)` = handles that thing and updates state
//! - `view(state)` = draws UI based on the current state
//!
//! The app repeats this forever:
//! **Message happens -> update changes state -> view redraws**
//!
//! # Current behavior (read-only)
//! - User adds one or more folder roots.
//! - "Scan Library" walks roots for `.mp3`, reads ID3 into `TrackRow`.
//! - Library is displayed as either:
//!   - Track View: flat list of tracks
//!   - Album View: grouped by (artist, album), with expandable albums
//! - Selecting a track populates the Inspector form.
//! - "Save edits" updates the in-memory `TrackRow` only (NO disk writes).
//!
//! # Not implemented yet
//! - Writing tags back to files
//! - Persistent DB/cache
//! - Audio playback
//!
//! # Architecture constraints (on purpose)
//! - UI layer calls `core::*` for scanning/tag reading.
//! - UI does not perform filesystem IO except validating user-entered root paths.
//!
//! # Concurrency model (aka “don’t freeze the app”)
//! - Scanning the disk can be slow.
//! - So we run scan work on a separate thread.
//! - When it finishes, it sends the results back as a `Message::ScanFinished(...)`
//!   so `update()` can safely apply the result.

mod core;

use iced::widget::{Column, button, column, row, scrollable, text, text_input};
use iced::{Length, Task};

use iced::futures::channel::oneshot;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::types::TrackRow;

/// Dev convenience:
/// If user didn’t add a folder root yet, scan `./test`.
/// This lets you test quickly without pasting a real path every time.
const TEST_ROOT: &str = "test";

/// Fixed UI heights (pixels) for scroll areas.
/// Keeping these fixed makes the layout predictable.
const ROOTS_HEIGHT: f32 = 120.0;
const LIST_HEIGHT: f32 = 460.0;


/// View mode: Albums vs Tracks
///
/// This controls how the left list is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    /// Grouped view: Artist+Album rows, expandable into tracks.
    Albums,
    /// Flat view: one big list of tracks.
    Tracks,
}

/// AlbumKey is used as the "grouping key" in Album View.
///
/// Why do we need this?
/// We want to group tracks into albums. A map needs a key.
/// We choose (artist, album) as the key.
///
/// NOTE: This is a simplification.
/// Real music libraries often need Album Artist, Disc #, etc.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AlbumKey {
    artist: String,
    album: String,
}


/// Inspector = the editable UI state
///
/// The inspector is the right-side “form” where you edit metadata.
/// We store what the user typed here, even if it’s not valid yet.
///
/// IMPORTANT:
/// - This is NOT writing to disk.
/// - This is NOT guaranteed to be valid.
/// - It’s just “draft text”, until Save.
#[derive(Debug, Default, Clone)]
struct InspectorDraft {
    /// Text box content for Title
    title: String,
    /// Text box content for Artist
    artist: String,
    /// Text box content for Album
    album: String,
    /// Track # is a String because users can type anything while editing
    /// (like "abc") and we only validate when saving.
    track_no: String,
    /// Same idea for Year.
    year: String,
}

/// Sonora is the app "state".
///
/// If you’re new to Rust: this struct is basically your “global variables”
/// (but stored nicely in one place).
///
/// Anything the UI needs to remember goes here.
struct Sonora {
    /// Status text shown near the top (errors, “Loaded 26 tracks”, etc.)
    status: String,

    /// True while scanning is running in another thread.
    /// We use this to disable buttons that shouldn't be pressed mid-scan.
    scanning: bool,

    // Folder roots UI
    /// Current text typed in the “Add folder path” box.
    root_input: String,
    /// Folder roots the user has added.
    /// We scan all of these when “Scan Library” is pressed.
    roots: Vec<PathBuf>,

    // Loaded tracks
    /// The current in-memory library.
    /// Filled by scanning & reading tags.
    tracks: Vec<TrackRow>,

    // UI structure
    /// Which view we’re in (Album View vs Track View)
    view_mode: ViewMode,
    /// Which album is selected (only relevant in Album View)
    selected_album: Option<AlbumKey>,
    /// Which track is selected. We store the index into `tracks`.
    selected_track: Option<usize>,

    // Inspector (right panel)
    /// Draft field values (what user typed)
    inspector: InspectorDraft,
    /// True after any edit, so we can enable/disable Save button
    inspector_dirty: bool,
}

impl Default for Sonora {
    /// Default state when the app first launches.
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

/// Message = “something happened”.
///
/// In Iced, you don't call functions directly from buttons.
/// Instead, buttons produce Messages.
///
/// Examples:
/// - user typed a character -> `RootInputChanged(...)`
/// - clicked Scan -> `ScanLibrary`
/// - scan finished -> `ScanFinished(...)`
///
/// Then `update()` matches on these and changes state accordingly.
#[derive(Debug, Clone)]
enum Message {
    // Roots UI
    /// User typed in root input box
    RootInputChanged(String),
    /// User pressed the “Add” button (or pressed Enter)
    AddRootPressed,
    /// User pressed “×” on a root row
    RemoveRoot(usize),

    // Scan
    /// User clicked “Scan Library”
    ScanLibrary,
    /// Background scan finished, returning either:
    /// - Ok((tracks, tag_failures))
    /// - Err(error_message)
    ScanFinished(Result<(Vec<TrackRow>, usize), String>),

    // View mode + selection
    /// Switch between Album View and Track View
    SetViewMode(ViewMode),
    /// Select an album (for expansion in Album View)
    SelectAlbum(AlbumKey),
    /// Select a specific track (index into `tracks`)
    SelectTrack(usize),

    // Inspector editing (fires as you type)
    EditTitle(String),
    EditArtist(String),
    EditAlbum(String),
    EditTrackNo(String),
    EditYear(String),

    /// “Save edits” button: applies inspector fields into `tracks[i]` in memory
    SaveInspectorToMemory,
    /// “Cancel edits” button: reload inspector from selected track (throw away draft)
    RevertInspector,
}

fn main() -> iced::Result {
    // `iced::application` glues together:
    // - initial state (Sonora::default)
    // - update function (logic)
    // - view function (UI layout)
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .run()
}

/// update() is the “brain”.
///
/// It takes:
/// - `state: &mut Sonora` = the app’s memory, mutable so we can change it
/// - `message: Message` = what just happened
///
/// It returns `Task<Message>`:
/// - Usually `Task::none()` (nothing async to do)
/// - Sometimes a background task that will later emit another Message
fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {

        // Roots (folders)

        Message::RootInputChanged(s) => {
            // User typed in the root input box → update stored text.
            state.root_input = s;
            Task::none()
        }

        Message::AddRootPressed => {
            // Read user input, trim whitespace.
            let input = state.root_input.trim();
            if input.is_empty() {
                return Task::none();
            }

            // Convert the text into a filesystem path type.
            let p = PathBuf::from(input);

            // Safety: only accept real directories.
            if !Path::new(input).is_dir() {
                state.status = format!("Not a folder: {}", p.display());
                return Task::none();
            }

            // Avoid duplicates.
            if state.roots.contains(&p) {
                state.status = format!("Already added: {}", p.display());
                state.root_input.clear();
                return Task::none();
            }

            // Save root and clear input.
            state.roots.push(p.clone());
            state.root_input.clear();
            state.status = format!("Added folder: {}", p.display());

            Task::none()
        }

        Message::RemoveRoot(i) => {
            // Don’t allow removing roots while scanning.
            if i < state.roots.len() && !state.scanning {
                let removed = state.roots.remove(i);
                state.status = format!("Removed folder: {}", removed.display());
            }
            Task::none()
        }


        // Scan

        Message::ScanLibrary => {
            // If already scanning, ignore the click.
            if state.scanning {
                return Task::none();
            }

            // Set scanning mode and clear old results.
            state.scanning = true;
            state.tracks.clear();
            state.status = "Scanning…".to_string();

            // Decide which roots we scan.
            // If none were added, default to ./test.
            let roots_to_scan = if state.roots.is_empty() {
                vec![PathBuf::from(TEST_ROOT)]
            } else {
                state.roots.clone()
            };

            // Kick off background work.
            //
            // Important idea:
            // - This returns immediately (UI stays responsive)
            // - When the background work completes, it produces Message::ScanFinished(...)
            Task::perform(
                async move {
                    // oneshot channel = a one-time "mailbox".
                    // The scan thread sends ONE result, then it’s done.
                    let (tx, rx) = oneshot::channel::<Result<(Vec<TrackRow>, usize), String>>();

                    std::thread::spawn(move || {
                        // This is the "heavy work" thread.
                        // It does disk scanning + tag reading.
                        let _ = tx.send(crate::core::scan_and_read_roots(roots_to_scan));
                    });

                    // Wait for the scan thread to send its result.
                    // If the thread dies without sending, show an error.
                    rx.await
                        .map_err(|_| "Scan thread dropped without returning".to_string())?
                },
                Message::ScanFinished,
            )
        }

        Message::ScanFinished(result) => {
            // Scan is done → re-enable UI.
            state.scanning = false;

            match result {
                Ok((mut rows, tag_failures)) => {
                    // Sorting makes output stable/predictable (nice for debugging).
                    rows.sort_by(|a, b| a.path.cmp(&b.path));

                    // Report results.
                    state.status = if tag_failures == 0 {
                        format!("Loaded {} tracks", rows.len())
                    } else {
                        format!(
                            "Loaded {} tracks ({} tag read failures)",
                            rows.len(),
                            tag_failures
                        )
                    };

                    // Store tracks in memory.
                    state.tracks = rows;

                    // After rescanning, any old selection might point to nonsense.
                    // Simplest safe behavior: clear selections.
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
            // Switch modes.
            state.view_mode = mode;

            // Clear selection + inspector for predictable behavior.
            state.selected_track = None;
            clear_inspector(state);

            if mode == ViewMode::Tracks {
                // Track view doesn’t use album selection.
                state.selected_album = None;
            }

            Task::none()
        }


        // Album selection

        Message::SelectAlbum(key) => {
            // Selecting an album expands it in Album View.
            state.selected_album = Some(key);

            // Selecting an album is NOT selecting a track.
            state.selected_track = None;
            clear_inspector(state);
            Task::none()
        }


        // Track selection

        Message::SelectTrack(i) => {
            // Select a track and load its metadata into inspector text boxes.
            if i < state.tracks.len() {
                state.selected_track = Some(i);
                load_inspector_from_track(state);
            }
            Task::none()
        }


        // Inspector typing

        // These all have the same pattern:
        // - update the draft field
        // - mark dirty so Save button becomes enabled
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
            // `Option` means “maybe there is a value”.
            // If nothing is selected, we can’t save.
            let Some(i) = state.selected_track else {
                state.status = "Select a track first.".to_string();
                return Task::none();
            };

            // Safety check: index must be valid.
            if i >= state.tracks.len() {
                return Task::none();
            }

            // Parse numeric fields only on Save.
            //
            // This is a good UX pattern:
            // - typing can be “invalid temporarily”
            // - saving must be valid
            let track_no = parse_optional_u32(&state.inspector.track_no)
                .map_err(|_| "Track # must be a number".to_string());

            let year = parse_optional_i32(&state.inspector.year)
                .map_err(|_| "Year must be a number".to_string());

            // Rust ownership note (baby version):
            // - `track_no` is a "Result"
            // - calling `.err()` normally can move values around
            // - `.as_ref()` lets us *peek* without consuming/moving it
            let track_err = track_no.as_ref().err();
            let year_err = year.as_ref().err();

            // If either parse failed, build a friendly status message.
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

            // At this point both are Ok(...), so unwrap is safe.
            let track_no = track_no.unwrap();
            let year = year.unwrap();

            // Update the selected TrackRow in memory.
            // `&mut` means we are allowed to modify it.
            let t = &mut state.tracks[i];

            // Clean strings: empty text box becomes None.
            // That keeps optional metadata consistent.
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
            // Throw away draft edits and reload from track.
            load_inspector_from_track(state);
            Task::none()
        }
    }
}


// View (UI)

/// view() is the “renderer”.
///
/// It takes the current state and returns a UI tree.
/// No logic here — just “how should it look?”
fn view(state: &Sonora) -> Column<'_, Message> {
    // ------- Roots UI -------
    // Text input: typing emits RootInputChanged messages.
    // Pressing Enter emits AddRootPressed.
    let root_input = text_input("Add folder path (ex: H:\\music)", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill);

    // When scanning, we disable Add so roots don’t change mid-scan.
    let add_btn = if state.scanning {
        button("Add")
    } else {
        button("Add").on_press(Message::AddRootPressed)
    };

    let add_row = row![root_input, add_btn].spacing(8);

    // Roots list: a scrollable column of (path + remove button)
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
    // Buttons that switch between Album View and Track View.
    // When you're already in a mode, the button becomes "disabled" (no on_press).
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
    // While scanning, label changes to Scanning…
    let scan_btn = if state.scanning {
        button("Scanning…")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    // ------- Main list (Albums or Tracks) -------
    // This chooses which list builder to use.
    let main_list = match state.view_mode {
        ViewMode::Tracks => build_tracks_list(state),
        ViewMode::Albums => build_albums_list(state),
    };

    // ------- Inspector panel (right side) -------
    let inspector_panel = build_inspector(state);

    // Body layout:
    // left side = scan button + main list
    // right side = inspector
    let body = row![
        column![scan_btn, main_list]
            .spacing(10)
            .width(Length::FillPortion(2)),
        inspector_panel.width(Length::FillPortion(1)),
    ]
    .spacing(12);

    // Whole page layout
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

/// Builds the scrollable Track View list.
/// Each row is a button; clicking it selects that track.
fn build_tracks_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    let mut list = column![];

    for (i, t) in state.tracks.iter().enumerate() {
        let label = format_track_one_line(t);

        // Small marker for selected track.
        let prefix = if state.selected_track == Some(i) { "▶ " } else { "  " };

        // Each row is a clickable button that sends SelectTrack(i).
        list = list.push(button(text(format!("{prefix}{label}"))).on_press(Message::SelectTrack(i)));
    }

    scrollable(list.spacing(6)).height(Length::Fixed(LIST_HEIGHT))
}


// Build Albums list (grouped)
// Click an album = expands its tracks

/// Builds the scrollable Album View list.
///
/// How it works:
/// 1) Group tracks into albums using a map.
/// 2) Draw each album as a button.
/// 3) If an album is selected, draw its tracks underneath.
fn build_albums_list(state: &Sonora) -> iced::widget::Scrollable<'_, Message> {
    // BTreeMap is like a HashMap but sorted by key.
    // Sorted output is nice for predictable UI.
    let mut groups: BTreeMap<AlbumKey, Vec<usize>> = BTreeMap::new();

    // Step 1: group tracks
    for (i, t) in state.tracks.iter().enumerate() {
        // If tag is missing, substitute "Unknown ..."
        let artist = t.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
        let album = t.album.clone().unwrap_or_else(|| "Unknown Album".to_string());

        let key = AlbumKey { artist, album };
        groups.entry(key).or_default().push(i);
    }

    let mut list = column![];

    // Step 2: draw each album + optional expanded tracks
    for (key, track_indexes) in groups {
        let is_selected_album = state.selected_album.as_ref() == Some(&key);

        let album_label = format!(
            "{} — {} ({} tracks)",
            key.artist,
            key.album,
            track_indexes.len()
        );

        // “▶” collapsed, “▼” expanded
        let album_prefix = if is_selected_album { "▼ " } else { "▶ " };

        // Album button toggles selection.
        list = list.push(
            button(text(format!("{album_prefix}{album_label}")))
                .on_press(Message::SelectAlbum(key.clone())),
        );

        // Step 3: if selected, show tracks under it
        if is_selected_album {
            for i in track_indexes {
                let t = &state.tracks[i];

                // Use filename if title tag is missing.
                let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));

                let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_else(|| "??".to_string());

                let track_line = format!("    #{track_no} — {title}");

                let prefix = if state.selected_track == Some(i) { "    ▶ " } else { "      " };

                list = list.push(
                    button(text(format!("{prefix}{track_line}")))
                        .on_press(Message::SelectTrack(i)),
                );
            }
        }
    }

    scrollable(list.spacing(6)).height(Length::Fixed(LIST_HEIGHT))
}


// Inspector UI (right panel)

/// Builds the right-side inspector panel.
///
/// The inspector is “draft editing”:
/// - it shows text inputs
/// - typing modifies `state.inspector`
/// - Save applies into `state.tracks[i]` (memory only)
fn build_inspector(state: &Sonora) -> Column<'_, Message> {
    // If nothing selected, show a friendly hint.
    let Some(i) = state.selected_track else {
        return column![
            text("Metadata inspector"),
            text("Select a track to edit metadata."),
            text("(Edits are not actually written to files for now.)"),
        ]
        .spacing(8);
    };

    // Safety: selected index must still be valid.
    if i >= state.tracks.len() {
        return column![text("Metadata inspector"), text("Invalid selection, rescan?")].spacing(8);
    }

    let t = &state.tracks[i];

    let path_line = format!("Path:\n{}", t.path.display());

    // Each input writes to the inspector draft via Message::EditX
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

    // Disable save if scanning or nothing changed.
    // (No on_press = “button does nothing”)
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


// Helpers

/// Copies data from the selected track into the inspector draft.
///
/// Why do we do this?
/// The inspector is separate from the track:
/// - track = “real data we loaded”
/// - inspector = “what the user is editing”
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
    state.inspector.artist = t.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
    state.inspector.album = t.album.clone().unwrap_or_else(|| "Unknown Album".to_string());

    state.inspector.track_no = t.track_no.map(|n| n.to_string()).unwrap_or_default();
    state.inspector.year = t.year.map(|y| y.to_string()).unwrap_or_default();

    state.inspector_dirty = false;
}

/// Clears the inspector draft (empty form).
fn clear_inspector(state: &mut Sonora) {
    state.inspector = InspectorDraft::default();
    state.inspector_dirty = false;
}

/// Gets filename without extension, used as a fallback title.
/// Example: `song.mp3` -> `song`
fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Title")
        .to_string()
}

/// Format TrackRow into a compact one-line label for Track View.
fn format_track_one_line(t: &TrackRow) -> String {
    let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    let artist = t.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
    let album = t.album.clone().unwrap_or_else(|| "Unknown Album".to_string());

    let track_no = t.track_no.map(|n| n.to_string()).unwrap_or_else(|| "??".to_string());

    format!("#{track_no} — {artist} — {title} ({album})")
}

/// Turn a string into Option<String>.
/// - empty string -> None
/// - non-empty -> Some(trimmed_string)
fn clean_optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse an optional u32 from a string.
/// - empty -> Ok(None)
/// - number -> Ok(Some(number))
/// - garbage -> Err(())
fn parse_optional_u32(s: &str) -> Result<Option<u32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<u32>().map(Some).map_err(|_| ())
}

/// Same idea as above, but for years (i32).
fn parse_optional_i32(s: &str) -> Result<Option<i32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| ())
}
