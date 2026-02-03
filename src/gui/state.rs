//! GUI state + message types.
//!
//! This file defines the "shape" of the app:
//! - what the app remembers ('Sonora')
//! - what can happen ('Message')
//! - small helper enums/structs used by the UI
//!
//! Keeping this separate makes it easier to navigate:
//! - update.rs handles behavior
//! - view.rs handles layout

use std::path::PathBuf;

use crate::core::types::TrackRow;

/// Dev convenience:
/// If user didn't add a folder root yet, scan './test'.
pub(crate) const TEST_ROOT: &str = "test";

/// Fixed UI heights (pixels) for scroll areas.
pub(crate) const ROOTS_HEIGHT: f32 = 120.0;
pub(crate) const LIST_HEIGHT: f32 = 460.0;

/// View mode: Albums vs Tracks
///
/// This controls how the left list is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    /// Grouped view: Artist + Album rows, expandable into tracks.
    Albums,
    /// Flat view: one big list of tracks.
    Tracks,
}

/// AlbumKey is used as the "grouping key" in Album View.
///
/// We group tracks into albums by (artist, album).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AlbumKey {
    pub(crate) artist: String,
    pub(crate) album: String,
}

/// Inspector draft = what the user is typing in the right panel.
///
/// IMPORTANT: this is NOT written to disk yet.
/// It's just the textbox contents until Save.
#[derive(Debug, Default, Clone)]
pub(crate) struct InspectorDraft {
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) album: String,
    pub(crate) track_no: String,
    pub(crate) year: String,
}

/// Sonora is the app "state".
///
/// Anything the UI needs to remember goes here.
pub(crate) struct Sonora {
    /// Status text shown near the top.
    pub(crate) status: String,

    /// True while scanning is running in another thread.
    pub(crate) scanning: bool,

    // Folder roots UI
    pub(crate) root_input: String,
    pub(crate) roots: Vec<PathBuf>,

    // Loaded tracks
    pub(crate) tracks: Vec<TrackRow>,

    // UI structure
    pub(crate) view_mode: ViewMode,
    pub(crate) selected_album: Option<AlbumKey>,
    pub(crate) selected_track: Option<usize>,

    // Inspector (right panel)
    pub(crate) inspector: InspectorDraft,
    pub(crate) inspector_dirty: bool,
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

/// Message = "something happened".
///
/// Buttons and text inputs emit these messages.
/// Then update.rs decides how the state changes.
#[derive(Debug, Clone)]
pub(crate) enum Message {
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
