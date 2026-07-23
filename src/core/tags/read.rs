//! core/tags/read.rs
//!
//! Reads ID3 metadata from an MP3 and converts it into a 'TrackRow'.
//!
//! Tag reading does not assign database identity. 'TrackRow.id' remains 'None'
//! until the scanning or DB layer associates the row with a cached track.

use std::path::PathBuf;

use id3::frame::Content;
use id3::{Tag, TagLike};

use super::super::probe::{
    AudioProperties, average_bitrate_kbps_from_audio_bytes, mp3_audio_bytes_excluding_id3,
    probe_audio_properties,
};
use super::super::types::TrackRow;
use super::util::{
    extract_year_from_release_date, normalize_release_date, parse_be_u64, parse_boolish,
};

const GROUPING_FRAME_ID: &str = "GRP1";
const CONTENT_GROUP_FRAME_ID: &str = "TIT1";

pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    let audio = probe_audio_properties(&path).unwrap_or_default();

    match Tag::read_from_path(&path) {
        Ok(tag) => (build_row_from_tag(path, &tag, audio), false),
        Err(_) => (build_probe_only_row(path, audio), true),
    }
}

fn build_probe_only_row(path: PathBuf, audio: AudioProperties) -> TrackRow {
    let mut row = TrackRow::empty(path.clone());
    apply_audio_properties(&mut row, audio);

    if row.bitrate_kbps.is_none() {
        if let (Some(bytes), Some(duration_ms)) =
            (mp3_audio_bytes_excluding_id3(&path), row.duration_ms)
        {
            row.bitrate_kbps = average_bitrate_kbps_from_audio_bytes(bytes, duration_ms);
        }
    }

    row
}

fn build_row_from_tag(path: PathBuf, tag: &Tag, audio: AudioProperties) -> TrackRow {
    let track_frame = text_frame(tag, "TRCK");
    let disc_frame = text_frame(tag, "TPOS");

    let (track_no_text, track_total_text) = split_counter_pair(track_frame.as_deref());
    let (disc_no_text, disc_total_text) = split_counter_pair(disc_frame.as_deref());

    let track_no = parse_counter_number(track_no_text.as_deref()).or_else(|| tag.track());
    let track_total = parse_counter_number(track_total_text.as_deref());

    let disc_no = parse_counter_number(disc_no_text.as_deref()).or_else(|| tag.disc());
    let disc_total = parse_counter_number(disc_total_text.as_deref());

    let release_date = text_frame(tag, "TDRC")
        .or_else(|| text_frame(tag, "TYER"))
        .and_then(|value| normalize_release_date(&value));

    let year = extract_year_from_release_date(release_date.as_deref());

    let artwork_count = tag
        .frames()
        .filter(|frame| frame.id() == "APIC" || frame.id() == "PIC")
        .count() as u32;

    let comment = first_comment(tag);
    let lyrics = first_lyrics(tag);

    let compilation = text_frame(tag, "TCMP")
        .and_then(|value| parse_boolish(&value))
        .or_else(|| user_text_value(tag, "COMPILATION").and_then(|value| parse_boolish(&value)));

    let (rating, popm_count) = popm_rating_and_count(tag);
    let pcnt_count = pcnt_count(tag);
    let play_count = popm_count.or(pcnt_count);

    let tlen_duration_ms =
        text_frame(tag, "TLEN").and_then(|value| value.trim().parse::<u32>().ok());

    let duration_ms = audio.duration_ms.or(tlen_duration_ms);

    let bitrate_kbps = audio.bitrate_kbps.or_else(|| {
        let duration_ms = duration_ms?;
        let audio_bytes = mp3_audio_bytes_excluding_id3(&path)?;

        average_bitrate_kbps_from_audio_bytes(audio_bytes, duration_ms)
    });

    TrackRow {
        id: None,
        path,

        title: tag
            .title()
            .map(str::to_owned)
            .or_else(|| text_frame(tag, "TIT2")),

        artist: tag
            .artist()
            .map(str::to_owned)
            .or_else(|| text_frame(tag, "TPE1")),

        album: tag
            .album()
            .map(str::to_owned)
            .or_else(|| text_frame(tag, "TALB")),

        album_artist: text_frame(tag, "TPE2"),
        composer: text_frame(tag, "TCOM"),

        track_no,
        track_total,
        disc_no,
        disc_total,

        track_no_text,
        track_total_text,
        disc_no_text,
        disc_total_text,

        release_date,
        year,
        genre: text_frame(tag, "TCON"),

        grouping: text_frame(tag, GROUPING_FRAME_ID),
        content_group: text_frame(tag, CONTENT_GROUP_FRAME_ID),
        comment,
        lyrics,
        lyricist: text_frame(tag, "TEXT"),

        conductor: text_frame(tag, "TPE3"),
        remixer: text_frame(tag, "TPE4"),
        publisher: text_frame(tag, "TPUB"),
        subtitle: text_frame(tag, "TIT3"),
        bpm: text_frame(tag, "TBPM").and_then(|value| value.trim().parse::<u32>().ok()),
        key: text_frame(tag, "TKEY"),
        mood: text_frame(tag, "TMOO"),
        language: text_frame(tag, "TLAN"),
        isrc: text_frame(tag, "TSRC"),
        encoder_settings: text_frame(tag, "TSSE"),
        encoded_by: text_frame(tag, "TENC"),
        copyright: text_frame(tag, "TCOP"),

        artwork_count,

        title_sort: text_frame(tag, "TSOT"),
        artist_sort: text_frame(tag, "TSOP"),
        album_sort: text_frame(tag, "TSOA"),
        album_artist_sort: text_frame(tag, "TSO2"),

        duration_ms,
        bitrate_kbps,
        sample_rate_hz: audio.sample_rate_hz,
        channels: audio.channels,

        rating,
        play_count,
        compilation,
    }
}

fn apply_audio_properties(row: &mut TrackRow, audio: AudioProperties) {
    row.duration_ms = audio.duration_ms;
    row.bitrate_kbps = audio.bitrate_kbps;
    row.sample_rate_hz = audio.sample_rate_hz;
    row.channels = audio.channels;
}

/// Split a 'TRCK' or 'TPOS' value while preserving each component's original
/// digit formatting.
///
/// Examples:
///
/// - '"01"' -> '("01", None)'
/// - '"01/09"' -> '("01", "09")'
/// - '"1/09"' -> '("1", "09")'
fn split_counter_pair(value: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(value) = value else {
        return (None, None);
    };

    let mut parts = value.splitn(2, '/');

    let number = normalize_counter_component(parts.next());
    let total = normalize_counter_component(parts.next());

    (number, total)
}

fn normalize_counter_component(value: Option<&str>) -> Option<String> {
    let value = value?.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_counter_number(value: Option<&str>) -> Option<u32> {
    value?.parse::<u32>().ok()
}

/// Return a best-effort string value from an ID3 frame.
///
/// Some text-like frames may not be represented as 'Content::Text', so links
/// are also accepted where appropriate.
fn text_frame(tag: &Tag, id: &str) -> Option<String> {
    let frame = tag.get(id)?;

    match frame.content() {
        Content::Text(value) => Some(value.clone()),
        Content::Link(value) => Some(value.clone()),
        _ => None,
    }
}

fn first_comment(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() == "COMM" {
            if let Content::Comment(comment) = frame.content() {
                return Some(comment.text.clone());
            }
        }
    }

    None
}

fn first_lyrics(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() == "USLT" {
            if let Content::Lyrics(lyrics) = frame.content() {
                return Some(lyrics.text.clone());
            }
        }
    }

    None
}

fn user_text_value(tag: &Tag, description: &str) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() != "TXXX" {
            continue;
        }

        if let Content::ExtendedText(extended_text) = frame.content() {
            if extended_text.description.eq_ignore_ascii_case(description) {
                return Some(extended_text.value.clone());
            }
        }
    }

    None
}

fn popm_rating_and_count(tag: &Tag) -> (Option<u8>, Option<u64>) {
    for frame in tag.frames() {
        if frame.id() == "POPM" {
            if let Content::Popularimeter(popularimeter) = frame.content() {
                return (Some(popularimeter.rating), Some(popularimeter.counter));
            }
        }
    }

    (None, None)
}

fn pcnt_count(tag: &Tag) -> Option<u64> {
    for frame in tag.frames() {
        if frame.id() != "PCNT" {
            continue;
        }

        let unknown = frame.content().to_unknown().ok()?;
        return parse_be_u64(unknown.as_ref().data.as_slice());
    }

    None
}
