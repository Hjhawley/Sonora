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
//!
//! Optimization:
//! - Precompute normalized query/sort fields once per dataset change.
//! - Rebuild only id lists when search/sort changes.

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

/// Precomputed normalized query/sort data for one `TrackRow`.
///
/// This is aligned by Vec index with `Sonora::tracks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryTrackCache {
    pub title: String,
    pub title_sort: String,
    pub artist: String,
    pub artist_sort: String,
    pub album: String,
    pub album_sort: String,
    pub album_artist: String,
    pub album_artist_sort: String,
    pub release_date: String,
    pub genre: String,
    pub search_blob: String,

    pub year: u32,
    pub disc_no: u32,
    pub track_no: u32,
    pub duration_ms: u64,
}

pub(crate) fn build_query_cache_rows(tracks: &[TrackRow]) -> Vec<QueryTrackCache> {
    tracks.iter().map(build_query_cache_row).collect()
}

/// Cached Track View ids for the current dataset/scope.
/// This is what the user sees in Track View.
pub(crate) fn track_ids_for_current_view(state: &Sonora) -> Vec<TrackId> {
    state.track_view_ids.clone()
}

/// Cached library playback ids for the current dataset/scope.
/// This respects the current sort field/direction, but intentionally ignores
/// search filtering so temporary narrowing does not redefine the queue.
pub(crate) fn track_ids_for_playback_queue(state: &Sonora) -> Vec<TrackId> {
    state.playback_queue_ids.clone()
}

/// Recompute Track View ids from precomputed rows using current search + sort.
pub(crate) fn build_track_view_ids(state: &Sonora) -> Vec<TrackId> {
    build_ids_with_options(state, true)
}

/// Recompute library playback ids from precomputed rows using current sort only.
pub(crate) fn build_playback_queue_ids(state: &Sonora) -> Vec<TrackId> {
    build_ids_with_options(state, false)
}

fn build_ids_with_options(state: &Sonora, apply_search: bool) -> Vec<TrackId> {
    let started = Instant::now();

    let mut items: Vec<(TrackId, usize)> = state
        .tracks
        .iter()
        .enumerate()
        .filter_map(|(i, track)| {
            let id = track.id?;
            let q = state.query_rows.get(i)?;

            if apply_search && !query_row_matches(q, &state.track_query) {
                return None;
            }

            Some((id, i))
        })
        .collect();

    let filter_ms = started.elapsed().as_secs_f64() * 1000.0;

    let sort_started = Instant::now();
    items.sort_by(|(id_a, idx_a), (id_b, idx_b)| {
        let qa = &state.query_rows[*idx_a];
        let qb = &state.query_rows[*idx_b];

        let base = compare_query_rows(qa, qb, state.track_query.sort_field)
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::Album))
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::TrackNo))
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::Title))
            .then_with(|| id_a.cmp(id_b));

        match state.track_query.sort_direction {
            SortDirection::Asc => base,
            SortDirection::Desc => base.reverse(),
        }
    });
    let sort_ms = sort_started.elapsed().as_secs_f64() * 1000.0;

    let ids: Vec<TrackId> = items.into_iter().map(|(id, _)| id).collect();

    let total_ms = started.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "[PERF][query::rebuild] apply_search={} total_tracks={} result_ids={} filter_ms={:.2} sort_ms={:.2} total_ms={:.2} search='{}'",
        apply_search,
        state.tracks.len(),
        ids.len(),
        filter_ms,
        sort_ms,
        total_ms,
        state.track_query.search_text
    );

    ids
}

fn build_query_cache_row(row: &TrackRow) -> QueryTrackCache {
    let title = normalized_title(row);
    let title_sort = normalized_title_sort(row);
    let artist = normalized_artist(row);
    let artist_sort = normalized_artist_sort(row);
    let album = normalized_album(row);
    let album_sort = normalized_album_sort(row);
    let album_artist = normalized_album_artist(row);
    let album_artist_sort = normalized_album_artist_sort(row);
    let release_date = normalized_release_date(row);
    let genre = normalized_genre(row);

    let search_blob = [
        title.as_str(),
        artist.as_str(),
        album.as_str(),
        album_artist.as_str(),
        genre.as_str(),
        release_date.as_str(),
    ]
    .join(" ");

    QueryTrackCache {
        title,
        title_sort,
        artist,
        artist_sort,
        album,
        album_sort,
        album_artist,
        album_artist_sort,
        release_date,
        genre,
        search_blob,
        year: row.year.unwrap_or(0).max(0) as u32,
        disc_no: row.disc_no.unwrap_or(0),
        track_no: row.track_no.unwrap_or(0),
        duration_ms: row.duration_ms.unwrap_or(0) as u64,
    }
}

#[inline]
fn query_row_matches(row: &QueryTrackCache, query: &TrackQuery) -> bool {
    let raw = query.search_text.trim();
    if raw.is_empty() {
        return true;
    }

    raw.split_whitespace()
        .map(normalize_for_match)
        .all(|term| !term.is_empty() && row.search_blob.contains(&term))
}

fn compare_query_rows(a: &QueryTrackCache, b: &QueryTrackCache, field: TrackSortField) -> Ordering {
    match field {
        TrackSortField::TrackNo => {
            (a.disc_no, a.track_no, &a.title).cmp(&(b.disc_no, b.track_no, &b.title))
        }

        TrackSortField::Title => {
            (&a.title_sort, &a.title, &a.artist).cmp(&(&b.title_sort, &b.title, &b.artist))
        }

        TrackSortField::Artist => (
            &a.artist_sort,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.artist_sort,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),

        TrackSortField::Album => (
            &a.album_sort,
            &a.album_artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.album_sort,
                &b.album_artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),

        TrackSortField::AlbumArtist => (
            &a.album_artist_sort,
            &a.album_artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.album_artist_sort,
                &b.album_artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),

        TrackSortField::ReleaseDate => (
            a.year,
            &a.release_date,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.year,
                &b.release_date,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),

        TrackSortField::Genre => (
            &a.genre, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.genre, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),

        TrackSortField::Duration => (
            a.duration_ms,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.duration_ms,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
    }
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
