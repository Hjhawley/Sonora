//! gui/columns.rs
//! Shared Track View column model.
//! source of truth for:
//! - what columns exist
//! - their labels
//! - their default widths
//! - which sort field they map to

use super::query::TrackSortField;
use super::view::constants::{
    TRACK_COL_ALBUM_ARTIST_SORT_W, TRACK_COL_ALBUM_ARTIST_W, TRACK_COL_ALBUM_SORT_W,
    TRACK_COL_ALBUM_W, TRACK_COL_ARTIST_SORT_W, TRACK_COL_ARTIST_W, TRACK_COL_ARTWORK_COUNT_W,
    TRACK_COL_BITRATE_W, TRACK_COL_BPM_W, TRACK_COL_CHANNELS_W, TRACK_COL_COMMENT_W,
    TRACK_COL_COMPILATION_W, TRACK_COL_COMPOSER_W, TRACK_COL_CONDUCTOR_W, TRACK_COL_COPYRIGHT_W,
    TRACK_COL_DISC_NO_W, TRACK_COL_DISC_TOTAL_W, TRACK_COL_ENCODED_BY_W,
    TRACK_COL_ENCODER_SETTINGS_W, TRACK_COL_GENRE_W, TRACK_COL_GROUPING_W, TRACK_COL_ISRC_W,
    TRACK_COL_KEY_W, TRACK_COL_LANGUAGE_W, TRACK_COL_LEN_W, TRACK_COL_LYRICIST_W,
    TRACK_COL_LYRICS_W, TRACK_COL_MARKER_W, TRACK_COL_MOOD_W, TRACK_COL_PATH_W, TRACK_COL_PLAYS_W,
    TRACK_COL_PUBLISHER_W, TRACK_COL_RATING_W, TRACK_COL_RELEASE_DATE_W, TRACK_COL_REMIXER_W,
    TRACK_COL_SAMPLE_RATE_W, TRACK_COL_SUBTITLE_W, TRACK_COL_TITLE_SORT_W, TRACK_COL_TITLE_W,
    TRACK_COL_TRACK_NO_W, TRACK_COL_TRACK_TOTAL_W, TRACK_COL_YEAR_W,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TrackColumn {
    Marker,
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

impl TrackColumn {
    pub const ALL: [Self; 42] = [
        Self::Marker,
        Self::Path,
        Self::TrackNo,
        Self::TrackTotal,
        Self::DiscNo,
        Self::DiscTotal,
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::AlbumArtist,
        Self::Composer,
        Self::ReleaseDate,
        Self::Year,
        Self::Genre,
        Self::Grouping,
        Self::Comment,
        Self::Lyrics,
        Self::Lyricist,
        Self::Conductor,
        Self::Remixer,
        Self::Publisher,
        Self::Subtitle,
        Self::Bpm,
        Self::Key,
        Self::Mood,
        Self::Language,
        Self::Isrc,
        Self::EncoderSettings,
        Self::EncodedBy,
        Self::Copyright,
        Self::ArtworkCount,
        Self::TitleSort,
        Self::ArtistSort,
        Self::AlbumSort,
        Self::AlbumArtistSort,
        Self::Duration,
        Self::Bitrate,
        Self::SampleRate,
        Self::Channels,
        Self::Rating,
        Self::PlayCount,
        Self::Compilation,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Marker => "",
            Self::Path => "Path",
            Self::TrackNo => "Track",
            Self::TrackTotal => "of",
            Self::DiscNo => "Disc",
            Self::DiscTotal => "of",
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::AlbumArtist => "Album Artist",
            Self::Composer => "Composer",
            Self::ReleaseDate => "Release Date",
            Self::Year => "Year",
            Self::Genre => "Genre",
            Self::Grouping => "Grouping",
            Self::Comment => "Comment",
            Self::Lyrics => "Lyrics",
            Self::Lyricist => "Lyricist",
            Self::Conductor => "Conductor",
            Self::Remixer => "Remixer",
            Self::Publisher => "Publisher",
            Self::Subtitle => "Subtitle",
            Self::Bpm => "BPM",
            Self::Key => "Key",
            Self::Mood => "Mood",
            Self::Language => "Language",
            Self::Isrc => "ISRC",
            Self::EncoderSettings => "Encoder",
            Self::EncodedBy => "Encoded By",
            Self::Copyright => "Copyright",
            Self::ArtworkCount => "Artwork",
            Self::TitleSort => "Title Sort",
            Self::ArtistSort => "Artist Sort",
            Self::AlbumSort => "Album Sort",
            Self::AlbumArtistSort => "Album Artist Sort",
            Self::Duration => "Length",
            Self::Bitrate => "kbps",
            Self::SampleRate => "Hz",
            Self::Channels => "Ch",
            Self::Rating => "Rating",
            Self::PlayCount => "Plays",
            Self::Compilation => "Comp",
        }
    }

    pub fn sort_field(self) -> Option<TrackSortField> {
        match self {
            Self::Marker => None,
            Self::Path => Some(TrackSortField::Path),
            Self::TrackNo => Some(TrackSortField::TrackNo),
            Self::TrackTotal => Some(TrackSortField::TrackTotal),
            Self::DiscNo => Some(TrackSortField::DiscNo),
            Self::DiscTotal => Some(TrackSortField::DiscTotal),
            Self::Title => Some(TrackSortField::Title),
            Self::Artist => Some(TrackSortField::Artist),
            Self::Album => Some(TrackSortField::Album),
            Self::AlbumArtist => Some(TrackSortField::AlbumArtist),
            Self::Composer => Some(TrackSortField::Composer),
            Self::ReleaseDate => Some(TrackSortField::ReleaseDate),
            Self::Year => Some(TrackSortField::Year),
            Self::Genre => Some(TrackSortField::Genre),
            Self::Grouping => Some(TrackSortField::Grouping),
            Self::Comment => Some(TrackSortField::Comment),
            Self::Lyrics => Some(TrackSortField::Lyrics),
            Self::Lyricist => Some(TrackSortField::Lyricist),
            Self::Conductor => Some(TrackSortField::Conductor),
            Self::Remixer => Some(TrackSortField::Remixer),
            Self::Publisher => Some(TrackSortField::Publisher),
            Self::Subtitle => Some(TrackSortField::Subtitle),
            Self::Bpm => Some(TrackSortField::Bpm),
            Self::Key => Some(TrackSortField::Key),
            Self::Mood => Some(TrackSortField::Mood),
            Self::Language => Some(TrackSortField::Language),
            Self::Isrc => Some(TrackSortField::Isrc),
            Self::EncoderSettings => Some(TrackSortField::EncoderSettings),
            Self::EncodedBy => Some(TrackSortField::EncodedBy),
            Self::Copyright => Some(TrackSortField::Copyright),
            Self::ArtworkCount => Some(TrackSortField::ArtworkCount),
            Self::TitleSort => Some(TrackSortField::TitleSort),
            Self::ArtistSort => Some(TrackSortField::ArtistSort),
            Self::AlbumSort => Some(TrackSortField::AlbumSort),
            Self::AlbumArtistSort => Some(TrackSortField::AlbumArtistSort),
            Self::Duration => Some(TrackSortField::Duration),
            Self::Bitrate => Some(TrackSortField::Bitrate),
            Self::SampleRate => Some(TrackSortField::SampleRate),
            Self::Channels => Some(TrackSortField::Channels),
            Self::Rating => Some(TrackSortField::Rating),
            Self::PlayCount => Some(TrackSortField::PlayCount),
            Self::Compilation => Some(TrackSortField::Compilation),
        }
    }

    pub fn default_width(self) -> f32 {
        match self {
            Self::Marker => TRACK_COL_MARKER_W,
            Self::Path => TRACK_COL_PATH_W,
            Self::TrackNo => TRACK_COL_TRACK_NO_W,
            Self::TrackTotal => TRACK_COL_TRACK_TOTAL_W,
            Self::DiscNo => TRACK_COL_DISC_NO_W,
            Self::DiscTotal => TRACK_COL_DISC_TOTAL_W,
            Self::Title => TRACK_COL_TITLE_W,
            Self::Artist => TRACK_COL_ARTIST_W,
            Self::Album => TRACK_COL_ALBUM_W,
            Self::AlbumArtist => TRACK_COL_ALBUM_ARTIST_W,
            Self::Composer => TRACK_COL_COMPOSER_W,
            Self::ReleaseDate => TRACK_COL_RELEASE_DATE_W,
            Self::Year => TRACK_COL_YEAR_W,
            Self::Genre => TRACK_COL_GENRE_W,
            Self::Grouping => TRACK_COL_GROUPING_W,
            Self::Comment => TRACK_COL_COMMENT_W,
            Self::Lyrics => TRACK_COL_LYRICS_W,
            Self::Lyricist => TRACK_COL_LYRICIST_W,
            Self::Conductor => TRACK_COL_CONDUCTOR_W,
            Self::Remixer => TRACK_COL_REMIXER_W,
            Self::Publisher => TRACK_COL_PUBLISHER_W,
            Self::Subtitle => TRACK_COL_SUBTITLE_W,
            Self::Bpm => TRACK_COL_BPM_W,
            Self::Key => TRACK_COL_KEY_W,
            Self::Mood => TRACK_COL_MOOD_W,
            Self::Language => TRACK_COL_LANGUAGE_W,
            Self::Isrc => TRACK_COL_ISRC_W,
            Self::EncoderSettings => TRACK_COL_ENCODER_SETTINGS_W,
            Self::EncodedBy => TRACK_COL_ENCODED_BY_W,
            Self::Copyright => TRACK_COL_COPYRIGHT_W,
            Self::ArtworkCount => TRACK_COL_ARTWORK_COUNT_W,
            Self::TitleSort => TRACK_COL_TITLE_SORT_W,
            Self::ArtistSort => TRACK_COL_ARTIST_SORT_W,
            Self::AlbumSort => TRACK_COL_ALBUM_SORT_W,
            Self::AlbumArtistSort => TRACK_COL_ALBUM_ARTIST_SORT_W,
            Self::Duration => TRACK_COL_LEN_W,
            Self::Bitrate => TRACK_COL_BITRATE_W,
            Self::SampleRate => TRACK_COL_SAMPLE_RATE_W,
            Self::Channels => TRACK_COL_CHANNELS_W,
            Self::Rating => TRACK_COL_RATING_W,
            Self::PlayCount => TRACK_COL_PLAYS_W,
            Self::Compilation => TRACK_COL_COMPILATION_W,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrackColumnState {
    pub kind: TrackColumn,
    pub visible: bool,
    pub width: f32,
}

impl TrackColumnState {
    pub fn new(kind: TrackColumn) -> Self {
        Self {
            kind,
            visible: true, // preserve current behavior for now
            width: kind.default_width(),
        }
    }
}

pub(crate) fn default_track_columns() -> Vec<TrackColumnState> {
    TrackColumn::ALL
        .into_iter()
        .map(TrackColumnState::new)
        .collect()
}
