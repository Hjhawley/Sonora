//! gui/state.rs
//!
//! GUI state + message vocabulary.
//!
//! This file is intentionally *data-only*:
//! - no view code (rendering)
//! - no update code (state transitions)
//! - no blocking IO
//!
//! If you’re looking for “how things change”, that lives in `gui/update/*`.
//! If you’re looking for “how things look”, that lives in `gui/view/*`.
//!
//! - **Selection, now playing, and cover cache are keyed by `TrackId`**
//! - We still keep `tracks: Vec<TrackRow>` for display order, but we do NOT treat indices as identity.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use crate::core::playback::{PlaybackController, PlayerEvent, start_playback};
use crate::core::types::{TrackId, TrackRow};

/// Dev convenience: if user didn’t add roots, scan `/test`.
pub(crate) const TEST_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test");

/// What the inspector shows when selected files disagree.
///
/// Semantics:
/// - In multi-select, if values differ, the field becomes `<keep>`
/// - On save, `<keep>` means “leave the file’s existing value as-is”
pub(crate) const KEEP_SENTINEL: &str = "<keep>";

/// Albums vs Tracks list mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Albums,
    Tracks,
}

/// Grouping key for Album View.
///
/// Important: This is a *UI grouping key*, not a DB key.
/// It’s derived from `TrackRow` values using your grouping rules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AlbumKey {
    pub album_artist: String,
    pub album: String,
}

/// Draft editable metadata (strings so the user can type anything).
///
/// This is an edit buffer, not the source of truth.
/// - Selection determines what we load into it.
/// - Save builds a "desired TrackRow" per target from this draft.
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
///
/// This is a stable identifier used by view → update messages.
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

/// App state.
///
/// Notes:
/// - `tracks` is display order; do not store identity as an index elsewhere.
/// - `TrackId` may be missing (`None`) pre-DB; selection logic should be robust.
///   (But once DB arrives, `None` should be treated as a bug.)
pub(crate) struct Sonora {
    // Status + lifecycle
    pub status: String,
    pub scanning: bool,

    // Roots
    pub root_input: String,
    pub roots: Vec<PathBuf>,

    // Library (display order)
    pub tracks: Vec<TrackRow>,

    /// Cache: `TrackId` → decoded cover image handle (for quick UI rendering).
    ///
    /// Why key by id?
    /// - Vec indices shift on rescans/sorts
    /// - id stays stable once DB arrives
    pub cover_cache: BTreeMap<TrackId, iced::widget::image::Handle>,

    // Playback (core handle + UI state)
    pub playback: Option<PlaybackController>,

    /// Receiver of engine events (polled via TickPlayback).
    pub playback_events: Option<RefCell<Receiver<PlayerEvent>>>,

    /// Which track is currently loaded/playing (stable id, not index).
    pub now_playing: Option<TrackId>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,

    /// While dragging the seek slider, keep a UI-only preview ratio here.
    /// On release, we commit it (send PlayerCommand::Seek).
    pub seek_preview_ratio: Option<f32>,

    // Selection / navigation
    pub view_mode: ViewMode,
    pub selected_album: Option<AlbumKey>,

    /// Multi-selection set of track ids (stable).
    pub selected_tracks: BTreeSet<TrackId>,

    /// Primary selection (stable id). Used as the "inspector anchor".
    pub selected_track: Option<TrackId>,

    /// For shift-click range selection (stable id).
    pub last_clicked_track: Option<TrackId>,

    // Inspector
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,
    pub saving: bool,

    /// For each field: are selected tracks "mixed" for this value?
    pub inspector_mixed: BTreeMap<InspectorField, bool>,

    // UI toggles
    pub show_extended: bool,
}

impl Sonora {
    /// Find the current display index for a given `TrackId`.
    ///
    /// This is a helper for bridging "stable identity" to "current ordering".
    /// Use it sparingly; prefer operating on ids in update logic.
    pub fn index_of_id(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == Some(id))
    }

    /// Get a reference to a track by id.
    pub fn track_by_id(&self, id: TrackId) -> Option<&TrackRow> {
        self.tracks.iter().find(|t| t.id == Some(id))
    }

    /// Get a mutable reference to a track by id.
    pub fn track_by_id_mut(&mut self, id: TrackId) -> Option<&mut TrackRow> {
        self.tracks.iter_mut().find(|t| t.id == Some(id))
    }
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
///
/// GUI emits these from view code. Update code consumes them.
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

    /// Select a track by stable id (not Vec index).
    SelectTrack(TrackId),

    // Cover art
    CoverLoaded(TrackId, Option<iced::widget::image::Handle>),

    // Playback controls (from UI)
    PlaySelected,

    /// Play a track by stable id (not Vec index).
    PlayTrack(TrackId),

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

    /// Save result for a single target track id.
    SaveFinished(TrackId, Result<TrackRow, String>),

    /// Save result for a batch.
    SaveFinishedBatch(Result<Vec<(TrackId, TrackRow)>, String>),

    RevertInspector,
}
