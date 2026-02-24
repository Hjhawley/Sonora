//! gui/state.rs
//!
//! GUI state + messages.
//! Pure data definitions used by update.rs + view.rs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::core::playback::{PlaybackController, PlayerEvent};
use crate::core::types::TrackRow;

/// Dev convenience: if user didn’t add roots, scan /test
pub(crate) const TEST_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test");

/// What the inspector shows when selected files disagree.
pub(crate) const KEEP_SENTINEL: &str = "<keep>";

/// Albums vs Tracks list mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Albums,
    Tracks,
}

/// Grouping key for Album View.
/// We prefer ALBUM ARTIST (TPE2), with fallback handled in view.rs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AlbumKey {
    pub album_artist: String,
    pub album: String,
}

/// Draft editable metadata (strings, so user can type anything).
#[derive(Debug, Default, Clone)]
pub(crate) struct InspectorDraft {
    // Standard (visible by default)
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
    pub genre: String,

    pub grouping: String,
    pub comment: String,
    pub lyrics: String,
    pub lyricist: String,

    // Extended (toggleable)
    pub date: String,
    pub conductor: String,
    pub remixer: String,
    pub publisher: String,
    pub subtitle: String,
    pub bpm: String,
    pub key: String,
    pub mood: String,
    pub language: String,
    pub isrc: String,
    pub encoder_settings: String,
    pub encoded_by: String,
    pub copyright: String,
}

/// Identifies which inspector field changed.
/// Used to collapse many Message::EditX variants into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InspectorField {
    // Standard (visible by default)
    Title,
    Artist,
    Album,
    AlbumArtist,
    Composer,

    TrackNo,
    TrackTotal,
    DiscNo,
    DiscTotal,

    Year,
    Genre,

    Grouping,
    Comment,
    Lyrics,
    Lyricist,

    // Extended (toggleable)
    Date,
    Conductor,
    Remixer,
    Publisher,
    Subtitle,
    Bpm,
    Key,
    Mood,
    Language,
    Isrc,
    EncoderSettings,
    EncodedBy,
    Copyright,
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

    /// Cache: track index -> decoded cover handle (for quick UI rendering)
    pub cover_cache: BTreeMap<usize, iced::widget::image::Handle>,

    // --------------------
    // Playback (core handle + UI state)
    // --------------------
    /// Core playback controller (channel sender wrapper). No rodio types in GUI.
    ///
    /// Lazy-initialized on first playback action so:
    /// - the app can start even if no audio device exists
    /// - we avoid startup cost if playback is never used
    pub playback: Option<PlaybackController>,

    /// Receiver of engine events (wire into Subscription later).
    pub playback_events: Option<Receiver<PlayerEvent>>,

    /// Track index currently “now playing” (into `tracks`)
    pub now_playing: Option<usize>,

    pub is_playing: bool,

    /// Playback position in milliseconds (UI progress bar)
    pub position_ms: u64,

    /// Duration in milliseconds (if known)
    pub duration_ms: Option<u64>,

    /// 0.0..=1.0
    pub volume: f32,

    // UI
    pub view_mode: ViewMode,
    pub selected_album: Option<AlbumKey>,

    /// Multi-select support: all selected track indices.
    pub selected_tracks: BTreeSet<usize>,

    /// The “primary” selection (used for inspector header / focus).
    pub selected_track: Option<usize>,

    /// Used later for shift-range selection (anchor). Safe to keep now.
    pub last_clicked_track: Option<usize>,

    // Inspector
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,
    pub saving: bool,

    /// Which inspector fields are currently “mixed” across the selection.
    pub inspector_mixed: BTreeMap<InspectorField, bool>,

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
            cover_cache: BTreeMap::new(),

            // Lazy init playback
            playback: None,
            playback_events: None,

            now_playing: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: None,
            volume: 1.0,

            view_mode: ViewMode::Tracks,
            selected_album: None,

            selected_tracks: BTreeSet::new(),
            selected_track: None,
            last_clicked_track: None,

            inspector: InspectorDraft::default(),
            inspector_dirty: false,
            saving: false,
            inspector_mixed: BTreeMap::new(),

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

    // Cover art
    CoverLoaded(usize, Option<iced::widget::image::Handle>),

    // --------------------
    // Playback controls (from UI)
    // --------------------
    /// Play the currently selected track (or do nothing if none).
    PlaySelected,

    /// Convenience: play a specific track index.
    PlayTrack(usize),

    TogglePlayPause,
    Next,
    Prev,

    /// Slider emits ratio (0..=1) currently.
    SeekTo(f32),

    /// 0.0..=1.0
    SetVolume(f32),

    // Playback events flowing from the engine via Subscription
    PlaybackEvent(PlayerEvent),

    // Inspector edits
    ToggleExtended(bool),
    InspectorChanged(InspectorField, String),

    // Actions
    SaveInspectorToFile,
    SaveFinished(usize, Result<TrackRow, String>),
    SaveFinishedBatch(Result<Vec<(usize, TrackRow)>, String>),
    RevertInspector,
}
