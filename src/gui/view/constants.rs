//! gui/view/constants.rs
//! View constants (layout/sizing)

#![allow(dead_code)]

// Window defaults (used by main.rs)
pub(crate) const WINDOW_W: f32 = 960.0;
pub(crate) const WINDOW_H: f32 = 720.0;

// Layout
pub(crate) const PLAYBACK_H: f32 = 108.0;
pub(crate) const PLAYBACK_COVER: f32 = 52.0;
pub(crate) const SIDEBAR_W: f32 = 260.0;
pub(crate) const EDITOR_W: f32 = 380.0;
pub(crate) const LABEL_W: f32 = 110.0;

// Text
pub(crate) const HEADER_TEXT: f32 = 14.0;
pub(crate) const ROW_TEXT: f32 = 14.0;

// Track list
pub(crate) const TRACK_ROW_H: f32 = 26.0;
pub(crate) const TRACK_ROW_VPAD: f32 = 2.0;
pub(crate) const TRACK_ROW_HPAD: f32 = 8.0;
pub(crate) const TRACK_LIST_SPACING: f32 = 1.0;
pub(crate) const TRACK_COL_SPACING: f32 = 10.0;

// Track view column widths
pub(crate) const TRACK_COL_MARKER_W: f32 = 24.0;
pub(crate) const TRACK_COL_PATH_W: f32 = 360.0;

pub(crate) const TRACK_COL_TRACK_NO_W: f32 = 44.0;
pub(crate) const TRACK_COL_TRACK_TOTAL_W: f32 = 52.0;
pub(crate) const TRACK_COL_DISC_NO_W: f32 = 44.0;
pub(crate) const TRACK_COL_DISC_TOTAL_W: f32 = 52.0;

pub(crate) const TRACK_COL_TITLE_W: f32 = 260.0;
pub(crate) const TRACK_COL_ARTIST_W: f32 = 190.0;
pub(crate) const TRACK_COL_ALBUM_W: f32 = 240.0;
pub(crate) const TRACK_COL_ALBUM_ARTIST_W: f32 = 180.0;
pub(crate) const TRACK_COL_COMPOSER_W: f32 = 180.0;

pub(crate) const TRACK_COL_RELEASE_DATE_W: f32 = 110.0;
pub(crate) const TRACK_COL_YEAR_W: f32 = 64.0;
pub(crate) const TRACK_COL_GENRE_W: f32 = 180.0;
pub(crate) const TRACK_COL_GROUPING_W: f32 = 180.0;
pub(crate) const TRACK_COL_COMMENT_W: f32 = 260.0;
pub(crate) const TRACK_COL_LYRICS_W: f32 = 300.0;
pub(crate) const TRACK_COL_LYRICIST_W: f32 = 180.0;
pub(crate) const TRACK_COL_CONDUCTOR_W: f32 = 180.0;
pub(crate) const TRACK_COL_REMIXER_W: f32 = 180.0;
pub(crate) const TRACK_COL_PUBLISHER_W: f32 = 180.0;
pub(crate) const TRACK_COL_SUBTITLE_W: f32 = 180.0;
pub(crate) const TRACK_COL_BPM_W: f32 = 64.0;
pub(crate) const TRACK_COL_KEY_W: f32 = 84.0;
pub(crate) const TRACK_COL_MOOD_W: f32 = 140.0;
pub(crate) const TRACK_COL_LANGUAGE_W: f32 = 110.0;
pub(crate) const TRACK_COL_ISRC_W: f32 = 130.0;
pub(crate) const TRACK_COL_ENCODER_SETTINGS_W: f32 = 180.0;
pub(crate) const TRACK_COL_ENCODED_BY_W: f32 = 160.0;
pub(crate) const TRACK_COL_COPYRIGHT_W: f32 = 220.0;

pub(crate) const TRACK_COL_ARTWORK_COUNT_W: f32 = 64.0;
pub(crate) const TRACK_COL_TITLE_SORT_W: f32 = 220.0;
pub(crate) const TRACK_COL_ARTIST_SORT_W: f32 = 220.0;
pub(crate) const TRACK_COL_ALBUM_SORT_W: f32 = 220.0;
pub(crate) const TRACK_COL_ALBUM_ARTIST_SORT_W: f32 = 220.0;

pub(crate) const TRACK_COL_LEN_W: f32 = 70.0;
pub(crate) const TRACK_COL_BITRATE_W: f32 = 72.0;
pub(crate) const TRACK_COL_SAMPLE_RATE_W: f32 = 76.0;
pub(crate) const TRACK_COL_CHANNELS_W: f32 = 52.0;
pub(crate) const TRACK_COL_RATING_W: f32 = 56.0;
pub(crate) const TRACK_COL_PLAYS_W: f32 = 64.0;
pub(crate) const TRACK_COL_COMPILATION_W: f32 = 72.0;

// Album grid
pub(crate) const ALBUM_GRID_MIN_COLS: usize = 1;
pub(crate) const ALBUM_GRID_SPACING_X: f32 = 22.0;
pub(crate) const ALBUM_GRID_SPACING_Y: f32 = 28.0;
pub(crate) const ALBUM_TILE_W: f32 = 180.0;
pub(crate) const ALBUM_TILE_COVER: f32 = 140.0;

// Album detail
pub(crate) const ALBUM_DETAIL_COVER: f32 = 260.0;
pub(crate) const ALBUM_DETAIL_TRACK_W_NO: f32 = 40.0;
pub(crate) const ALBUM_DETAIL_TRACK_W_LEN: f32 = 64.0;

// Artwork
pub(crate) const COVER_BIG: f32 = 220.0;
