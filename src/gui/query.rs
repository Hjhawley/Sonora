//! gui/query.rs
//!
//! Pure query + sorting logic for Track View.
//!
//! Important:
//! - This does NOT mutate app state.
//! - This derives display order and playback order from canonical in-memory
//!   'TrackRow's.
//! - Track View selection/navigation should use display order.
//! - Library playback queue should use sort order, but ignore search text.

use std::cmp::Ordering;
use std::time::Instant;

use crate::core::types::{TrackId, TrackRow};

use super::state::Sonora;
use super::util::filename_stem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackSortField {
    TrackNo,
    Title,
    Artist,
    Album,
    AlbumArtist,
    ReleaseDate,
    Genre,
    Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    #[inline]
    pub fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrackQuery {
    pub search_text: String,
    pub sort_field: TrackSortField,
    pub sort_direction: SortDirection,
}

impl Default for TrackQuery {
    fn default() -> Self {
        Self {
            search_text: String::new(),
            sort_field: TrackSortField::Title,
            sort_direction: SortDirection::Asc,
        }
    }
}

/// Ordered + filtered Track View ids for the current dataset/scope.
/// This is what the user sees in Track View.
pub(crate) fn track_ids_for_current_view(state: &Sonora) -> Vec<TrackId> {
    track_ids_with_options(state, true)
}

/// Ordered library playback ids for the current dataset/scope.
/// This respects the current sort field/direction, but intentionally ignores
/// search filtering so temporary narrowing does not redefine the queue.
pub(crate) fn track_ids_for_playback_queue(state: &Sonora) -> Vec<TrackId> {
    track_ids_with_options(state, false)
}

fn track_ids_with_options(state: &Sonora, apply_search: bool) -> Vec<TrackId> {
    let started = Instant::now();

    let mut ids: Vec<TrackId> = state
        .tracks
        .iter()
        .filter_map(|t| t.id)
        .filter(|id| {
            let Some(row) = state.track_by_id(*id) else {
                return false;
            };

            if apply_search {
                row_matches_query(row, &state.track_query)
            } else {
                true
            }
        })
        .collect();

    let filtered_ms = started.elapsed().as_secs_f64() * 1000.0;

    let sort_started = Instant::now();
    ids.sort_by(|a, b| compare_track_ids(state, *a, *b));
    let sort_ms = sort_started.elapsed().as_secs_f64() * 1000.0;

    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][query] apply_search={} total_tracks={} result_ids={} filter_ms={:.2} sort_ms={:.2} total_ms={:.2} search='{}'",
        apply_search,
        state.tracks.len(),
        ids.len(),
        filtered_ms,
        sort_ms,
        total_ms,
        state.track_query.search_text
    );

    ids
}

#[inline]
pub(crate) fn row_matches_query(row: &TrackRow, query: &TrackQuery) -> bool {
    let raw = query.search_text.trim();
    if raw.is_empty() {
        return true;
    }

    let haystack = searchable_blob(row);
    raw.split_whitespace()
        .map(normalize_for_match)
        .all(|term| !term.is_empty() && haystack.contains(&term))
}

fn compare_track_ids(state: &Sonora, a: TrackId, b: TrackId) -> Ordering {
    let Some(ta) = state.track_by_id(a) else {
        return a.cmp(&b);
    };
    let Some(tb) = state.track_by_id(b) else {
        return a.cmp(&b);
    };

    let base = compare_rows_by_field(ta, tb, state.track_query.sort_field)
        .then_with(|| compare_rows_by_field(ta, tb, TrackSortField::Album))
        .then_with(|| compare_rows_by_field(ta, tb, TrackSortField::TrackNo))
        .then_with(|| compare_rows_by_field(ta, tb, TrackSortField::Title))
        .then_with(|| a.cmp(&b));

    match state.track_query.sort_direction {
        SortDirection::Asc => base,
        SortDirection::Desc => base.reverse(),
    }
}

fn compare_rows_by_field(a: &TrackRow, b: &TrackRow, field: TrackSortField) -> Ordering {
    match field {
        TrackSortField::TrackNo => (
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::Title => (
            normalized_title_sort(a),
            normalized_title(a),
            normalized_artist(a),
        )
            .cmp(&(
                normalized_title_sort(b),
                normalized_title(b),
                normalized_artist(b),
            )),

        TrackSortField::Artist => (
            normalized_artist_sort(a),
            normalized_artist(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                normalized_artist_sort(b),
                normalized_artist(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::Album => (
            normalized_album_sort(a),
            normalized_album_artist(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                normalized_album_sort(b),
                normalized_album_artist(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::AlbumArtist => (
            normalized_album_artist_sort(a),
            normalized_album_artist(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                normalized_album_artist_sort(b),
                normalized_album_artist(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::ReleaseDate => (
            a.year.unwrap_or(0),
            normalized_release_date(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                b.year.unwrap_or(0),
                normalized_release_date(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::Genre => (
            normalized_genre(a),
            normalized_artist(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                normalized_genre(b),
                normalized_artist(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),

        TrackSortField::Duration => (
            a.duration_ms.unwrap_or(0),
            normalized_artist(a),
            normalized_album(a),
            a.disc_no.unwrap_or(0),
            a.track_no.unwrap_or(0),
            normalized_title(a),
        )
            .cmp(&(
                b.duration_ms.unwrap_or(0),
                normalized_artist(b),
                normalized_album(b),
                b.disc_no.unwrap_or(0),
                b.track_no.unwrap_or(0),
                normalized_title(b),
            )),
    }
}

fn searchable_blob(row: &TrackRow) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(s) = row.title.as_deref() {
        parts.push(s.to_string());
    } else {
        parts.push(filename_stem(&row.path));
    }

    if let Some(s) = row.artist.as_deref() {
        parts.push(s.to_string());
    }

    if let Some(s) = row.album.as_deref() {
        parts.push(s.to_string());
    }

    if let Some(s) = row.album_artist.as_deref() {
        parts.push(s.to_string());
    }

    if let Some(s) = row.genre.as_deref() {
        parts.push(s.to_string());
    }

    if let Some(s) = row.release_date.as_deref() {
        parts.push(s.to_string());
    }

    normalize_for_match(&parts.join(" "))
}

#[inline]
fn normalize_for_match(s: &str) -> String {
    s.trim().to_lowercase()
}

#[inline]
fn opt_norm(s: Option<&str>) -> String {
    s.unwrap_or("").trim().to_lowercase()
}

#[inline]
fn normalized_title(row: &TrackRow) -> String {
    match row.title.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => opt_norm(Some(s)),
        None => normalize_for_match(&filename_stem(&row.path)),
    }
}

#[inline]
fn normalized_artist(row: &TrackRow) -> String {
    opt_norm(
        row.artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(Some("Unknown")),
    )
}

#[inline]
fn normalized_album(row: &TrackRow) -> String {
    opt_norm(
        row.album
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(Some("Unknown")),
    )
}

#[inline]
fn normalized_album_artist(row: &TrackRow) -> String {
    opt_norm(
        row.album_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| row.artist.as_deref().filter(|s| !s.trim().is_empty()))
            .or(Some("Unknown")),
    )
}

#[inline]
fn normalized_release_date(row: &TrackRow) -> String {
    opt_norm(row.release_date.as_deref())
}

#[inline]
fn normalized_genre(row: &TrackRow) -> String {
    opt_norm(row.genre.as_deref())
}

#[inline]
fn normalized_title_sort(row: &TrackRow) -> String {
    opt_norm(
        row.title_sort
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(row.title.as_deref().filter(|s| !s.trim().is_empty())),
    )
}

#[inline]
fn normalized_artist_sort(row: &TrackRow) -> String {
    opt_norm(
        row.artist_sort
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(row.artist.as_deref().filter(|s| !s.trim().is_empty())),
    )
}

#[inline]
fn normalized_album_sort(row: &TrackRow) -> String {
    opt_norm(
        row.album_sort
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(row.album.as_deref().filter(|s| !s.trim().is_empty())),
    )
}

#[inline]
fn normalized_album_artist_sort(row: &TrackRow) -> String {
    opt_norm(
        row.album_artist_sort
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(row
                .album_artist
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| row.artist.as_deref().filter(|s| !s.trim().is_empty()))),
    )
}
