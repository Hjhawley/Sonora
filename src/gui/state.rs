//! gui/state.rs
//!
//! GUI state + messages.
//! Pure data definitions used by update.rs + view.rs.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::core::playback::{PlaybackController, PlayerEvent, start_playback};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InspectorField {
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
    pub playback: Option<PlaybackController>,

    /// Receiver of engine events (polled via TickPlayback).
    pub playback_events: Option<RefCell<Receiver<PlayerEvent>>>,

    pub now_playing: Option<usize>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,

    /// While dragging the seek slider, keep a UI-only preview ratio here.
    /// On release, we commit it (send PlayerCommand::Seek).
    pub seek_preview_ratio: Option<f32>,

    // UI
    pub view_mode: ViewMode,
    pub selected_album: Option<AlbumKey>,
    pub selected_tracks: BTreeSet<usize>,
    pub selected_track: Option<usize>,
    pub last_clicked_track: Option<usize>,

    // Inspector
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,
    pub saving: bool,
    pub inspector_mixed: BTreeMap<InspectorField, bool>,

    // UI toggles
    pub show_extended: bool,
}

impl Default for Sonora {
    fn default() -> Self {
        let (playback_controller, playback_events) = start_playback();

        Self {
            status: "Add a folder, then Scan.".to_string(),
            scanning: false,

            root_input: String::new(),
            roots: Vec::new(),

            tracks: Vec::new(),
            cover_cache: BTreeMap::new(),

            playback: Some(playback_controller),
            playback_events: Some(RefCell::new(playback_events)),

            now_playing: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: None,
            volume: 1.0,

            seek_preview_ratio: None,

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
    Noop,

    /// Periodic tick to drain playback events.
    TickPlayback,

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

    // Playback controls (from UI)
    PlaySelected,
    PlayTrack(usize),
    TogglePlayPause,
    Next,
    Prev,

    /// Seek slider changed (preview only; does NOT command the engine)
    SeekTo(f32),

    /// Seek slider released (commit the seek)
    SeekCommit,

    SetVolume(f32),

    // (optional path; still supported)
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
