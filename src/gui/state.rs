//! gui/state.rs
//!
//! GUI state + message vocabulary.
//! - no view code (rendering)
//! - no update code (state transitions)
//! - no blocking IO except light startup library restore
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

use super::columns::{TrackColumn, TrackColumnState, default_track_columns};
use super::query::{
    QueryTrackCache, TrackQuery, TrackSortField, build_playback_queue_ids, build_query_cache_rows,
    build_track_view_ids,
};

/// What the inspector shows when selected files have different metadata for the same fields.
/// - This is a UI display string, not the true source of mixed-state meaning.
/// - Real mixed-state is tracked structurally in 'Sonora::inspector_mixed'.
/// - The view renders this in a distinct color so it cannot be mistaken for a literal value.
pub(crate) const MIXED_SENTINEL: &str = "<mixed>";

#[inline]
pub(crate) fn mixed_display_string() -> &'static str {
    MIXED_SENTINEL
}

#[inline]
pub(crate) fn is_mixed_display_value(s: &str) -> bool {
    s.trim() == MIXED_SENTINEL
}

/// Tracks vs Albums is a layout choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Albums,
    Tracks,
}

/// Library / Hidden / Missing is a dataset/scope choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryScope {
    Library,
    Hidden,
    Missing,
}

/// Playback ordering policy.
/// - Normal = display order
/// - Shuffle = persistent shuffled queue order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayOrder {
    Normal,
    Shuffle,
}

/// Repeat policy.
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
/// This is a UI grouping key, not a DB key.
/// It’s derived from 'TrackRow' values using grouping rules.
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
/// Mixed-state is tracked separately in 'Sonora::inspector_mixed'.
/// When a field is mixed, the draft typically contains 'MIXED_SENTINEL' for display.
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

    pub release_date: String,
    pub genre: String,

    pub grouping: String,
    pub comment: String,
    pub lyrics: String,
    pub lyricist: String,

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

impl InspectorDraft {
    #[inline]
    pub fn set_mixed(field: &mut String) {
        *field = MIXED_SENTINEL.to_string();
    }
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

    ReleaseDate,
    Genre,

    Grouping,
    Comment,
    Lyrics,
    Lyricist,

    Conductor,
    Remixer,
    Publisher,
    EncoderSettings,
    EncodedBy,
    Subtitle,
    Bpm,
    Key,
    Mood,
    Language,
    Isrc,
    Copyright,
}

#[derive(Debug, Clone)]
pub(crate) struct PickedArtwork {
    pub bytes: Vec<u8>,
    pub mime: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ArtworkEdit {
    Unchanged,
    Remove,
    Replace {
        bytes: Vec<u8>,
        mime: String,
        preview: iced::widget::image::Handle,
    },
}

impl Default for ArtworkEdit {
    fn default() -> Self {
        Self::Unchanged
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveColumnResize {
    pub column: TrackColumn,
    pub anchor_x: Option<f32>,
    pub start_width: f32,
}

pub(crate) struct Sonora {
    // Status + lifecycle
    pub status: String,
    pub scanning: bool,

    // Roots
    pub root_input: String,
    pub roots: Vec<PathBuf>,

    // Library (canonical rows for current scope)
    pub tracks: Vec<TrackRow>,

    /// Cache: 'TrackId' -> current 'Vec' index.
    pub track_index: BTreeMap<TrackId, usize>,

    /// Cache: one precomputed normalized query/sort row per entry in 'tracks'.
    pub query_rows: Vec<QueryTrackCache>,

    /// Cache: 'AlbumKey' -> ordered list of 'TrackId's in that album group.
    pub album_groups: BTreeMap<AlbumKey, Vec<TrackId>>,

    /// Cache: 'TrackId' -> decoded cover image handle (for quick UI rendering).
    pub cover_cache: BTreeMap<TrackId, iced::widget::image::Handle>,

    // Track View query / display controls
    pub track_query: TrackQuery,

    /// Cached Track View ids after applying current search + sort.
    pub track_view_ids: Vec<TrackId>,

    /// Cached library playback ids after applying current sort only
    /// (search intentionally ignored).
    pub playback_queue_ids: Vec<TrackId>,

    /// Track View virtualization state.
    pub tracks_scroll_offset_y: f32,
    pub tracks_viewport_height: f32,
    pub tracks_overscan_rows: usize,

    /// Track View column config.
    /// Order in this Vec is display order.
    /// Visibility/width live here so the table can eventually support
    /// hide/reorder/resize without hardcoded private view state.
    pub track_columns: Vec<TrackColumnState>,

    /// Live Track View column resize interaction, if any.
    pub active_column_resize: Option<ActiveColumnResize>,

    // Playback (core handle + UI state)
    pub playback: Option<PlaybackController>,

    /// Receiver of engine events (polled via 'TickPlayback').
    pub playback_events: Option<RefCell<Receiver<PlayerEvent>>>,

    /// Current engine playback session id.
    pub active_playback_id: Option<u64>,

    /// True after issuing a PlayFile/Seek that should produce a fresh 'Started' event.
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
    pub selection_anchor: Option<TrackId>,
    pub last_clicked_track: Option<TrackId>,

    /// Current modifier keys, updated from keyboard events.
    pub modifiers: iced::keyboard::Modifiers,

    /// Album-view double click bookkeeping.
    pub last_album_press: Option<(AlbumPressTarget, Instant)>,

    // Inspector
    pub inspector_open: bool,
    pub inspector: InspectorDraft,
    pub inspector_dirty: bool,
    pub saving: bool,
    pub inspector_art_edit: ArtworkEdit,

    /// 'true' means "selected files disagree on this field".
    /// This is the authoritative mixed-state signal.
    /// The inspector draft may display 'MIXED_SENTINEL', but save logic should rely
    /// on this structure rather than trusting the raw string.
    pub inspector_mixed: BTreeMap<InspectorField, bool>,
}

impl Sonora {
    /// Real application startup constructor.
    pub fn new() -> Self {
        let (playback_controller, playback_events) = start_playback();

        let (saved_volume, roots) = (|| -> Result<(Option<f32>, Vec<PathBuf>), String> {
            let db_path = core::db::default_db_path()?;
            let db = core::db::Db::open(&db_path)?;
            Ok((db.load_volume()?, db.load_roots()?))
        })()
        .unwrap_or((None, Vec::new()));

        let saved_volume = saved_volume.unwrap_or(1.0).clamp(0.0, 1.0);

        let (tracks, status) = match core::load_visible_tracks_from_db() {
            Ok((tracks, failures)) => {
                if tracks.is_empty() {
                    if roots.is_empty() {
                        (
                            tracks,
                            "Add a folder, then Scan. Existing library will appear here once scanned."
                                .to_string(),
                        )
                    } else {
                        (
                            tracks,
                            "No tracks loaded yet. Press Scan to index your saved library folders."
                                .to_string(),
                        )
                    }
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
            roots,

            tracks,

            track_index: BTreeMap::new(),
            query_rows: Vec::new(),
            album_groups: BTreeMap::new(),
            cover_cache: BTreeMap::new(),

            track_query: TrackQuery::default(),
            track_columns: default_track_columns(),
            active_column_resize: None,
            track_view_ids: Vec::new(),
            playback_queue_ids: Vec::new(),

            tracks_scroll_offset_y: 0.0,
            tracks_viewport_height: 0.0,
            tracks_overscan_rows: 5,

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
            selection_anchor: None,
            last_clicked_track: None,

            modifiers: iced::keyboard::Modifiers::default(),

            last_album_press: None,

            inspector_open: false,
            inspector: InspectorDraft::default(),
            inspector_dirty: false,
            saving: false,
            inspector_art_edit: ArtworkEdit::Unchanged,
            inspector_mixed: BTreeMap::new(),
        };

        s.rebuild_library_caches();

        if let Some(controller) = &s.playback {
            controller.send(crate::core::playback::PlayerCommand::SetVolume(s.volume));
        }

        s
    }

    #[inline]
    pub fn has_selection(&self) -> bool {
        self.selected_track.is_some() || !self.selected_tracks.is_empty()
    }

    #[inline]
    pub fn inspector_has_keyboard_priority(&self) -> bool {
        self.inspector_open
    }

    #[inline]
    pub fn playback_shortcuts_enabled(&self) -> bool {
        !self.inspector_has_keyboard_priority()
    }

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
    pub fn representative_cover_track_id(&self, key: &AlbumKey) -> Option<TrackId> {
        let ids = self.album_groups.get(key)?;

        ids.iter()
            .copied()
            .find(|id| self.track_by_id(*id).is_some_and(|t| t.artwork_count > 0))
            .or_else(|| ids.first().copied())
    }

    pub fn rebuild_library_derived_state(&mut self) {
        self.track_index.clear();
        self.album_groups.clear();

        let mut visible_ids: Vec<TrackId> = Vec::new();

        for (i, t) in self.tracks.iter().enumerate() {
            let Some(id) = t.id else {
                continue;
            };
            self.track_index.insert(id, i);
            visible_ids.push(id);
        }

        for t in &self.tracks {
            let Some(id) = t.id else {
                continue;
            };

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

        if matches!(self.selected_track, Some(id) if !self.track_index.contains_key(&id)) {
            self.selected_track = None;
        }

        self.selected_tracks
            .retain(|id| self.track_index.contains_key(id));

        if matches!(self.selection_anchor, Some(id) if !self.track_index.contains_key(&id)) {
            self.selection_anchor = None;
        }

        if !self.has_selection() {
            self.inspector_open = false;
        }
    }

    pub fn rebuild_track_query_index(&mut self) {
        self.query_rows = build_query_cache_rows(&self.tracks);
    }

    pub fn rebuild_track_query_caches(&mut self) {
        self.track_view_ids = build_track_view_ids(self);
        self.playback_queue_ids = build_playback_queue_ids(self);
    }

    #[inline]
    pub fn rebuild_library_caches(&mut self) {
        self.rebuild_library_derived_state();
        self.rebuild_track_query_index();
        self.rebuild_track_query_caches();
    }
}

impl Default for Sonora {
    fn default() -> Self {
        Self::new()
    }
}

/// Message = "something happened"
#[derive(Debug, Clone)]
pub(crate) enum Message {
    Noop,

    /// Periodic tick to drain playback events.
    TickPlayback,

    /// Global keyboard events.
    KeyboardEvent(iced::keyboard::Event),

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

    // Track View query / sorting
    TrackSearchChanged(String),
    ClearTrackSearch,
    SetTrackSortField(TrackSortField),

    StartTrackColumnResize(TrackColumn),
    UpdateTrackColumnResize {
        cursor_x: f32,
    },
    EndTrackColumnResize,

    /// Track View scroll/viewport updates for row virtualization.
    TracksScrolled {
        offset_y: f32,
        viewport_height: f32,
    },

    // View + selection
    SetViewMode(ViewMode),
    TrackPressed(TrackId),

    // Album-view click handling
    AlbumTilePressed(AlbumKey),
    AlbumHeaderPressed(AlbumKey),
    AlbumTrackPressed(AlbumKey, TrackId),

    // Cover art
    CoverLoaded(TrackId, Option<iced::widget::image::Handle>),

    // Inspector artwork
    ChooseInspectorArtwork,
    InspectorArtworkChosen(Result<Option<PickedArtwork>, String>),
    RemoveInspectorArtwork,
    ExtractInspectorArtwork,
    InspectorArtworkExtracted(Result<Option<PathBuf>, String>),

    // Playback controls (from UI)
    PlayAlbum(AlbumKey),
    TogglePlayPause,
    ToggleShuffle,
    CycleRepeatMode,
    Next,
    Prev,

    SeekTo(f32),
    SeekCommit,
    SetVolume(f32),

    // Inspector edits
    InspectorChanged(InspectorField, String),
    CloseInspector,

    // Actions
    SaveInspectorToFile,
    SaveFinished(TrackId, Result<TrackRow, String>),
    SaveFinishedBatch(Result<Vec<(TrackId, TrackRow)>, String>),

    // Sonora-only visibility / DB record actions
    HideSelected,
    UnhideSelected,
    DeleteSelectedFromSonora,
}
