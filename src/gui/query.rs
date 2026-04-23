//! gui/query.rs
//!
//! Pure query + sorting logic for Track View.
//! - This does NOT mutate app state.
//! - This derives display order and playback order from in-memory 'TrackRow's.
//! - Track View selection/navigation should use display order.
//! - Library playback queue should use sort order, but ignore search text.
//! - Precompute normalized query/sort fields once per dataset change.
//! - Rebuild only id lists when search/sort changes.

use std::cmp::Ordering;
use std::time::Instant;

use crate::core::types::{TrackId, TrackRow};

use super::state::Sonora;
use super::util::filename_stem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrackSortField {
    Path,

    TrackNo,
    TrackTotal,
    DiscNo,
    DiscTotal,

    Title,
    Artist,
    Album,
    AlbumArtist,
    Composer,

    ReleaseDate,
    Year,
    Genre,
    Grouping,
    Comment,
    Lyrics,
    Lyricist,
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

    ArtworkCount,
    TitleSort,
    ArtistSort,
    AlbumSort,
    AlbumArtistSort,

    Duration,
    Bitrate,
    SampleRate,
    Channels,
    Rating,
    PlayCount,
    Compilation,
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

/// Precomputed normalized query/sort data for one 'TrackRow'.
/// This is aligned by Vec index with 'Sonora::tracks'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryTrackCache {
    pub path: String,

    pub title: String,
    pub title_sort: String,
    pub artist: String,
    pub artist_sort: String,
    pub album: String,
    pub album_sort: String,
    pub album_artist: String,
    pub album_artist_sort: String,
    pub composer: String,

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
    pub key_name: String,
    pub mood: String,
    pub language: String,
    pub isrc: String,
    pub encoder_settings: String,
    pub encoded_by: String,
    pub copyright: String,

    pub search_blob: String,

    pub year: i32,
    pub track_no: u32,
    pub track_total: u32,
    pub disc_no: u32,
    pub disc_total: u32,
    pub bpm: u32,
    pub artwork_count: u32,
    pub duration_ms: u64,
    pub bitrate_kbps: u32,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub rating: u8,
    pub play_count: u64,
    pub compilation: bool,
}

pub(crate) fn build_query_cache_rows(tracks: &[TrackRow]) -> Vec<QueryTrackCache> {
    tracks.iter().map(build_query_cache_row).collect()
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
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::DiscNo))
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::TrackNo))
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::Title))
            .then_with(|| compare_query_rows(qa, qb, TrackSortField::Path))
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
    let path = normalize_path(row);

    let title = normalized_title(row);
    let title_sort = normalized_title_sort(row);
    let artist = normalized_artist(row);
    let artist_sort = normalized_artist_sort(row);
    let album = normalized_album(row);
    let album_sort = normalized_album_sort(row);
    let album_artist = normalized_album_artist(row);
    let album_artist_sort = normalized_album_artist_sort(row);
    let composer = opt_norm(row.composer.as_deref());

    let release_date = normalized_release_date(row);
    let genre = normalized_genre(row);
    let grouping = opt_norm(row.grouping.as_deref());
    let comment = opt_norm(row.comment.as_deref());
    let lyrics = opt_norm(row.lyrics.as_deref());
    let lyricist = opt_norm(row.lyricist.as_deref());
    let conductor = opt_norm(row.conductor.as_deref());
    let remixer = opt_norm(row.remixer.as_deref());
    let publisher = opt_norm(row.publisher.as_deref());
    let subtitle = opt_norm(row.subtitle.as_deref());
    let key_name = opt_norm(row.key.as_deref());
    let mood = opt_norm(row.mood.as_deref());
    let language = opt_norm(row.language.as_deref());
    let isrc = opt_norm(row.isrc.as_deref());
    let encoder_settings = opt_norm(row.encoder_settings.as_deref());
    let encoded_by = opt_norm(row.encoded_by.as_deref());
    let copyright = opt_norm(row.copyright.as_deref());

    let bpm = row.bpm.unwrap_or(0);
    let artwork_count = row.artwork_count;
    let duration_ms = row.duration_ms.unwrap_or(0) as u64;
    let bitrate_kbps = row.bitrate_kbps.unwrap_or(0);
    let sample_rate_hz = row.sample_rate_hz.unwrap_or(0);
    let channels = row.channels.unwrap_or(0);
    let rating = row.rating.unwrap_or(0);
    let play_count = row.play_count.unwrap_or(0);
    let compilation = row.compilation.unwrap_or(false);

    let year = row.year.unwrap_or(0);
    let track_no = row.track_no.unwrap_or(0);
    let track_total = row.track_total.unwrap_or(0);
    let disc_no = row.disc_no.unwrap_or(0);
    let disc_total = row.disc_total.unwrap_or(0);

    let search_blob = [
        path.as_str(),
        title.as_str(),
        title_sort.as_str(),
        artist.as_str(),
        artist_sort.as_str(),
        album.as_str(),
        album_sort.as_str(),
        album_artist.as_str(),
        album_artist_sort.as_str(),
        composer.as_str(),
        release_date.as_str(),
        genre.as_str(),
        grouping.as_str(),
        comment.as_str(),
        lyrics.as_str(),
        lyricist.as_str(),
        conductor.as_str(),
        remixer.as_str(),
        publisher.as_str(),
        subtitle.as_str(),
        key_name.as_str(),
        mood.as_str(),
        language.as_str(),
        isrc.as_str(),
        encoder_settings.as_str(),
        encoded_by.as_str(),
        copyright.as_str(),
        &year.to_string(),
        &track_no.to_string(),
        &track_total.to_string(),
        &disc_no.to_string(),
        &disc_total.to_string(),
        &bpm.to_string(),
        &artwork_count.to_string(),
        &duration_ms.to_string(),
        &bitrate_kbps.to_string(),
        &sample_rate_hz.to_string(),
        &channels.to_string(),
        &rating.to_string(),
        &play_count.to_string(),
        if compilation { "true" } else { "false" },
    ]
    .join(" ");

    QueryTrackCache {
        path,

        title,
        title_sort,
        artist,
        artist_sort,
        album,
        album_sort,
        album_artist,
        album_artist_sort,
        composer,

        release_date,
        genre,
        grouping,
        comment,
        lyrics,
        lyricist,
        conductor,
        remixer,
        publisher,
        subtitle,
        key_name,
        mood,
        language,
        isrc,
        encoder_settings,
        encoded_by,
        copyright,

        search_blob,

        year,
        track_no,
        track_total,
        disc_no,
        disc_total,
        bpm,
        artwork_count,
        duration_ms,
        bitrate_kbps,
        sample_rate_hz,
        channels,
        rating,
        play_count,
        compilation,
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
        TrackSortField::Path => a.path.cmp(&b.path),

        TrackSortField::TrackNo => {
            (a.disc_no, a.track_no, &a.title).cmp(&(b.disc_no, b.track_no, &b.title))
        }
        TrackSortField::TrackTotal => (a.track_total, a.disc_no, a.track_no, &a.title).cmp(&(
            b.track_total,
            b.disc_no,
            b.track_no,
            &b.title,
        )),
        TrackSortField::DiscNo => {
            (a.disc_no, a.track_no, &a.title).cmp(&(b.disc_no, b.track_no, &b.title))
        }
        TrackSortField::DiscTotal => (a.disc_total, a.disc_no, a.track_no, &a.title).cmp(&(
            b.disc_total,
            b.disc_no,
            b.track_no,
            &b.title,
        )),

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
        TrackSortField::Composer => (
            &a.composer,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.composer,
                &b.artist,
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
        TrackSortField::Year => (
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
        TrackSortField::Grouping => (
            &a.grouping,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.grouping,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Comment => (
            &a.comment, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.comment, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::Lyrics => (
            &a.lyrics, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.lyrics, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::Lyricist => (
            &a.lyricist,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.lyricist,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Conductor => (
            &a.conductor,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.conductor,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Remixer => (
            &a.remixer, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.remixer, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::Publisher => (
            &a.publisher,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.publisher,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Subtitle => (
            &a.subtitle,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.subtitle,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Bpm => (a.bpm, &a.artist, &a.album, a.disc_no, a.track_no, &a.title)
            .cmp(&(b.bpm, &b.artist, &b.album, b.disc_no, b.track_no, &b.title)),
        TrackSortField::Key => (
            &a.key_name,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.key_name,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Mood => (
            &a.mood, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.mood, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::Language => (
            &a.language,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.language,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Isrc => (
            &a.isrc, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                &b.isrc, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::EncoderSettings => (
            &a.encoder_settings,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.encoder_settings,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::EncodedBy => (
            &a.encoded_by,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.encoded_by,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Copyright => (
            &a.copyright,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                &b.copyright,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),

        TrackSortField::ArtworkCount => (
            a.artwork_count,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.artwork_count,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::TitleSort => {
            (&a.title_sort, &a.title, &a.artist).cmp(&(&b.title_sort, &b.title, &b.artist))
        }
        TrackSortField::ArtistSort => (
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
        TrackSortField::AlbumSort => (
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
        TrackSortField::AlbumArtistSort => (
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
        TrackSortField::Bitrate => (
            a.bitrate_kbps,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.bitrate_kbps,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::SampleRate => (
            a.sample_rate_hz,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.sample_rate_hz,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Channels => (
            a.channels, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                b.channels, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::Rating => (
            a.rating, &a.artist, &a.album, a.disc_no, a.track_no, &a.title,
        )
            .cmp(&(
                b.rating, &b.artist, &b.album, b.disc_no, b.track_no, &b.title,
            )),
        TrackSortField::PlayCount => (
            a.play_count,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.play_count,
                &b.artist,
                &b.album,
                b.disc_no,
                b.track_no,
                &b.title,
            )),
        TrackSortField::Compilation => (
            a.compilation,
            &a.artist,
            &a.album,
            a.disc_no,
            a.track_no,
            &a.title,
        )
            .cmp(&(
                b.compilation,
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
fn normalize_path(row: &TrackRow) -> String {
    row.path.to_string_lossy().trim().to_lowercase()
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
            .or(Some("unknown")),
    )
}

#[inline]
fn normalized_album(row: &TrackRow) -> String {
    opt_norm(
        row.album
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or(Some("unknown")),
    )
}

#[inline]
fn normalized_album_artist(row: &TrackRow) -> String {
    opt_norm(
        row.album_artist
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| row.artist.as_deref().filter(|s| !s.trim().is_empty()))
            .or(Some("unknown")),
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
