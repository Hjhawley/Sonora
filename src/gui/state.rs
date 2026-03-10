//! gui/state.rs
//!
//! GUI state + message vocabulary.
//!
//! This file is intentionally *data-only*:
//! - no view code (rendering)
//! - no update code (state transitions)
//! - no blocking IO except light startup library restore
//!
//! If you’re looking for "how things change", that lives in 'gui/update/*'.
//! If you’re looking for "how things look", that lives in 'gui/view/*'.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::core;
use crate::core::playback::{PlaybackController, PlayerEvent, start_playback};
use crate::core::types::{TrackId, TrackRow};

/// Dev: if user didn’t add roots, scan '/test'
pub(crate) const TEST_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test");

/// What the inspector shows when selected files disagree.
///
/// Semantics:
/// - In multi-select, if values differ, the field becomes '<keep>'
/// - On save, '<keep>' means "leave the file’s existing value as-is"
pub(crate) const KEEP_SENTINEL: &str = "<keep>";

/// Tracks vs Albums is a layout choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Albums,
    Tracks,
}

/// Library vs Hidden is a dataset/scope choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryScope {
    Library,
    Hidden,
}

/// Playback ordering policy.
///
/// - Normal = display order
/// - Shuffle = persistent shuffled queue order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayOrder {
    Normal,
    Shuffle,
}

/// Repeat policy.
///
/// - Off = stop at end of queue
/// - All = wrap entire queue
/// - One = replay current track
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepeatMode {
    Off,
    All,
    One,
}

/// Explicit playback queue scope.
///
/// This is intentionally separate from:
/// - current view
/// - current selection
/// - current metadata editing target
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlaybackContext {
    Library,
    Album(AlbumKey),
}

/// Grouping key for Album View.
///
/// Important: This is a *UI grouping key*, not a DB key.
/// It’s derived from 'TrackRow' values using your grouping rules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AlbumKey {
    pub album_artist: String,
    pub album: String,
}

/// Used for local double-click detection in Album View.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AlbumPressTarget {
    Tile(AlbumKey),
    Header(AlbumKey),
    Track(AlbumKey, TrackId),
}

/// Draft editable metadata (strings so the user can type anything).
#[derive(Debug, Default, Clone)]
pub(crate) struct InspectorDraft {
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

pub(crate) struct Sonora {
    // Status + lifecycle
    pub status: String,
    pub scanning: bool,

    // Roots
    pub root_input: String,
    pub roots: Vec<PathBuf>,

    // Library (display order)
    pub tracks: Vec<TrackRow>,

    /// Cache: 'TrackId' -> current Vec index.
    pub track_index: BTreeMap<TrackId, usize>,

    /// Cache: 'AlbumKey' -> ordered list of 'TrackId's in that album group.
    pub album_groups: BTreeMap<AlbumKey, Vec<TrackId>>,

    /// Cache: 'TrackId' -> decoded cover image handle (for quick UI rendering).
    pub cover_cache: BTreeMap<TrackId, iced::widget::image::Handle>,

    // Playback (core handle + UI state)
    pub playback: Option<PlaybackController>,

    /// Receiver of engine events (polled via TickPlayback).
    pub playback_events: Option<RefCell<Receiver<PlayerEvent>>>,

    /// Current engine playback session id.
    pub active_playback_id: Option<u64>,

    /// True after issuing a PlayFile/Seek that should produce a fresh Started event.
    /// While this is true, stale non-Started transport events are ignored.
    pub awaiting_started: bool,

    /// Which track is currently loaded/playing (stable id, not index).
    pub now_playing: Option<TrackId>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub volume: f32,

    /// While dragging the seek slider, keep a UI-only preview ratio here.
    pub seek_preview_ratio: Option<f32>,

    // Playback policy
    pub play_order: PlayOrder,
    pub repeat_mode: RepeatMode,
    pub playback_context: PlaybackContext,

    /// Persistent queue order used only when 'play_order == PlayOrder::Shuffle'.
    pub shuffled_ids: Vec<TrackId>,

    // Selection / navigation
    pub view_mode: ViewMode,
    pub library_scope: LibraryScope,

    /// In Album View:
    /// - 'None' => album grid
    /// - 'Some(key)' => album detail screen
    pub selected_album: Option<AlbumKey>,

    pub selected_tracks: BTreeSet<TrackId>,
    pub selected_track: Option<TrackId>,
    pub last_clicked_track: Option<TrackId>,

    /// Album-view double click bookkeeping.
    pub last_album_press: Option<(AlbumPressTarget, Instant)>,

    // Inspector
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,
    pub saving: bool,
    pub inspector_mixed: BTreeMap<InspectorField, bool>,

    // UI toggles
    pub show_extended: bool,
}

impl Sonora {
    #[inline]
    pub fn index_of_id(&self, id: TrackId) -> Option<usize> {
        self.track_index.get(&id).copied()
    }

    #[inline]
    pub fn track_by_id(&self, id: TrackId) -> Option<&TrackRow> {
        let i = self.index_of_id(id)?;
        self.tracks.get(i)
    }

    #[inline]
    pub fn track_by_id_mut(&mut self, id: TrackId) -> Option<&mut TrackRow> {
        let i = self.index_of_id(id)?;
        self.tracks.get_mut(i)
    }

    #[inline]
    pub fn representative_track_id_for_album(&self, key: &AlbumKey) -> Option<TrackId> {
        self.album_groups
            .get(key)
            .and_then(|ids| ids.first().copied())
    }

    pub fn rebuild_library_caches(&mut self) {
        self.track_index.clear();
        self.album_groups.clear();

        let mut visible_ids: Vec<TrackId> = Vec::new();

        for (i, t) in self.tracks.iter().enumerate() {
            let Some(id) = t.id else { continue };
            self.track_index.insert(id, i);
            visible_ids.push(id);
        }

        for t in self.tracks.iter() {
            let Some(id) = t.id else { continue };

            let album_artist = t
                .album_artist
                .clone()
                .or_else(|| t.artist.clone())
                .unwrap_or_else(|| "Unknown Artist".to_string());

            let album = t
                .album
                .clone()
                .unwrap_or_else(|| "Unknown Album".to_string());

            self.album_groups
                .entry(AlbumKey {
                    album_artist,
                    album,
                })
                .or_default()
                .push(id);
        }

        // Keep the shuffle queue valid across scans/scope changes:
        // - drop ids that no longer exist
        // - dedupe defensively
        // - append any newly visible ids at the end
        let valid_ids: BTreeSet<TrackId> = visible_ids.iter().copied().collect();

        let mut seen: BTreeSet<TrackId> = BTreeSet::new();
        self.shuffled_ids
            .retain(|id| valid_ids.contains(id) && seen.insert(*id));

        let mut queued: BTreeSet<TrackId> = self.shuffled_ids.iter().copied().collect();
        for id in visible_ids {
            if queued.insert(id) {
                self.shuffled_ids.push(id);
            }
        }

        if matches!(&self.playback_context, PlaybackContext::Album(key) if !self.album_groups.contains_key(key))
        {
            self.playback_context = PlaybackContext::Library;
        }

        if matches!(&self.selected_album, Some(key) if !self.album_groups.contains_key(key)) {
            self.selected_album = None;
        }
    }
}

impl Default for Sonora {
    fn default() -> Self {
        let (playback_controller, playback_events) = start_playback();

        let saved_volume = (|| -> Result<Option<f32>, String> {
            let db_path = core::db::default_db_path()?;
            let db = core::db::Db::open(&db_path)?;
            db.load_volume()
        })()
        .unwrap_or(None)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

        let (tracks, status) = match core::load_visible_tracks_from_db() {
            Ok((tracks, failures)) => {
                if tracks.is_empty() {
                    (
                        tracks,
                        "Add a folder, then Scan. Existing library will appear here once scanned."
                            .to_string(),
                    )
                } else if failures == 0 {
                    (
                        tracks.clone(),
                        format!("Loaded {} tracks from library.", tracks.len()),
                    )
                } else {
                    (
                        tracks.clone(),
                        format!(
                            "Loaded {} tracks from library ({} tag read failures).",
                            tracks.len(),
                            failures
                        ),
                    )
                }
            }
            Err(e) => (Vec::new(), format!("Library DB unavailable: {e}")),
        };

        let mut s = Self {
            status,
            scanning: false,

            root_input: String::new(),
            roots: Vec::new(),

            tracks,

            track_index: BTreeMap::new(),
            album_groups: BTreeMap::new(),
            cover_cache: BTreeMap::new(),

            playback: Some(playback_controller),
            playback_events: Some(RefCell::new(playback_events)),

            active_playback_id: None,
            awaiting_started: false,

            now_playing: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: None,
            volume: saved_volume,

            seek_preview_ratio: None,

            play_order: PlayOrder::Normal,
            repeat_mode: RepeatMode::Off,
            playback_context: PlaybackContext::Library,
            shuffled_ids: Vec::new(),

            view_mode: ViewMode::Tracks,
            library_scope: LibraryScope::Library,
            selected_album: None,

            selected_tracks: BTreeSet::new(),
            selected_track: None,
            last_clicked_track: None,
            last_album_press: None,

            inspector: InspectorDraft::default(),
            inspector_dirty: false,
            saving: false,
            inspector_mixed: BTreeMap::new(),

            show_extended: false,
        };

        s.rebuild_library_caches();

        if let Some(controller) = &s.playback {
            controller.send(crate::core::playback::PlayerCommand::SetVolume(s.volume));
        }

        s
    }
}

/// Message = "something happened"
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

    // Library scope / reloading
    SetLibraryScope(LibraryScope),
    ScopeLoaded(Result<(LibraryScope, Vec<TrackRow>, usize), String>),

    // View + selection
    SetViewMode(ViewMode),
    SelectAlbum(AlbumKey),
    SelectTrack(TrackId),

    // Album-view click handling (single-click select, double-click play)
    AlbumTilePressed(AlbumKey),
    AlbumHeaderPressed(AlbumKey),
    AlbumTrackPressed(AlbumKey, TrackId),

    // Cover art
    CoverLoaded(TrackId, Option<iced::widget::image::Handle>),

    // Playback controls (from UI)
    PlaySelected,
    PlayTrack(TrackId),
    PlayAlbum(AlbumKey),
    PlayAlbumFromTrack(AlbumKey, TrackId),
    TogglePlayPause,
    ToggleShuffle,
    CycleRepeatMode,
    Next,
    Prev,

    SeekTo(f32),
    SeekCommit,
    SetVolume(f32),

    PlaybackEvent(PlayerEvent),

    // Inspector edits
    ToggleExtended(bool),
    InspectorChanged(InspectorField, String),

    // Actions
    SaveInspectorToFile,
    SaveFinished(TrackId, Result<TrackRow, String>),
    SaveFinishedBatch(Result<Vec<(TrackId, TrackRow)>, String>),
    RevertInspector,

    // Sonora-only visibility
    HideSelected,
    UnhideSelected,
}
