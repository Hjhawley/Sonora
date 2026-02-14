//! GUI state + messages.
//! Pure data definitions used by update.rs + view.rs.

use std::path::PathBuf;

use crate::core::types::TrackRow;

/// Dev convenience: if user didn’t add roots, scan ./test
pub(crate) const TEST_ROOT: &str = "test";

/// Fixed UI heights (pixels)
pub(crate) const ROOTS_HEIGHT: f32 = 120.0;
pub(crate) const LIST_HEIGHT: f32 = 460.0;

/// Albums vs Tracks list mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Albums,
    Tracks,
}

/// Grouping key for Album View.
/// Use ALBUM ARTIST (TPE2) first.
/// Fallback behavior happens in view.rs (Unknown Artist, etc).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AlbumKey {
    pub album_artist: String,
    pub album: String,
}

/// Draft editable metadata (strings, so user can type anything).
#[derive(Debug, Default, Clone)]
pub(crate) struct InspectorDraft {
    // Core tags
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub composer: String,

    pub track_no: String,
    pub track_total: String,
    pub disc_no: String,
    pub disc_total: String,

    pub year: String,
    pub date: String,
    pub genre: String,

    // Extended (toggleable)
    pub lyricist: String,
    pub conductor: String,
    pub remixer: String,
    pub publisher: String,
    pub grouping: String,
    pub subtitle: String,
    pub bpm: String,
    pub key: String,
    pub mood: String,
    pub language: String,
    pub isrc: String,
    pub encoder_settings: String,
    pub encoded_by: String,
    pub copyright: String,

    pub comment: String,
    pub lyrics: String,
}

/// App state
pub(crate) struct Sonora {
    pub status: String,
    pub scanning: bool,

    // Roots
    pub root_input: String,
    pub roots: Vec<PathBuf>,

    // Library
    pub tracks: Vec<TrackRow>,

    // UI
    pub view_mode: ViewMode,
    pub selected_album: Option<AlbumKey>,
    pub selected_track: Option<usize>,

    // Inspector
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,

    // UI toggles
    pub show_extended: bool,
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

            show_extended: false,
        }
    }
}

/// Message = “something happened”.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    // Roots
    RootInputChanged(String),
    AddRootPressed,
    RemoveRoot(usize),

    // Scan
    ScanLibrary,
    ScanFinished(Result<(Vec<TrackRow>, usize), String>),

    // View + selection
    SetViewMode(ViewMode),
    SelectAlbum(AlbumKey),
    SelectTrack(usize),

    // Inspector edits (core)
    EditTitle(String),
    EditArtist(String),
    EditAlbum(String),
    EditAlbumArtist(String),
    EditComposer(String),

    EditTrackNo(String),
    EditTrackTotal(String),
    EditDiscNo(String),
    EditDiscTotal(String),

    EditYear(String),
    EditDate(String),
    EditGenre(String),

    // Inspector edits (extended)
    ToggleExtended(bool),

    EditLyricist(String),
    EditConductor(String),
    EditRemixer(String),
    EditPublisher(String),
    EditGrouping(String),
    EditSubtitle(String),
    EditBpm(String),
    EditKey(String),
    EditMood(String),
    EditLanguage(String),
    EditIsrc(String),
    EditEncoderSettings(String),
    EditEncodedBy(String),
    EditCopyright(String),
    EditComment(String),
    EditLyrics(String),

    // Actions
    SaveInspectorToMemory,
    RevertInspector,
}
