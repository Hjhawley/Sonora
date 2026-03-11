//! core/tags/read.rs
//! Read ID3 tags from an MP3 and convert them into a `TrackRow`.
//!
//! - Tag reading does NOT assign identity.
//! - `TrackRow.id` is set by the scanning / DB layer.
//! - This module therefore always returns `id: None`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use id3::frame::Content;
use id3::{Tag, TagLike};

use super::super::types::TrackRow;
use super::util::{parse_be_u64, parse_boolish, parse_slash_pair_u32};

pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    match Tag::read_from_path(&path) {
        Ok(tag) => (build_row_from_tag(path, &tag), false),
        Err(_) => (TrackRow::empty(path), true),
    }
}

fn build_row_from_tag(path: PathBuf, tag: &Tag) -> TrackRow {
    let (track_no_from_text, track_total) =
        parse_slash_pair_u32(text_frame(tag, "TRCK").as_deref());
    let (disc_no_from_text, disc_total) = parse_slash_pair_u32(text_frame(tag, "TPOS").as_deref());

    let track_no = tag.track().or(track_no_from_text);
    let disc_no = tag.disc().or(disc_no_from_text);

    let date = text_frame(tag, "TDRC").or_else(|| text_frame(tag, "TYER"));
    let year = tag.year();

    let artwork_count = tag
        .frames()
        .filter(|f| f.id() == "APIC" || f.id() == "PIC")
        .count() as u32;

    let comment = first_comment(tag);
    let lyrics = first_lyrics(tag);

    let user_text = collect_user_text(tag);
    let urls = collect_urls(tag);

    let compilation = text_frame(tag, "TCMP")
        .and_then(|s| parse_boolish(&s))
        .or_else(|| user_text.get("COMPILATION").and_then(|s| parse_boolish(s)));

    let (rating, popm_count) = popm_rating_and_count(tag);
    let pcnt_count = pcnt_count(tag);
    let play_count = popm_count.or(pcnt_count);

    let duration_ms = text_frame(tag, "TLEN").and_then(|s| s.trim().parse::<u32>().ok());

    let extra_text = collect_extra_text(tag);

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

        year,
        date,

        genre: text_frame(tag, "TCON"),

        grouping: text_frame(tag, "TIT1"),
        comment,
        lyrics,
        lyricist: text_frame(tag, "TEXT"),

        conductor: text_frame(tag, "TPE3"),
        remixer: text_frame(tag, "TPE4"),
        publisher: text_frame(tag, "TPUB"),
        subtitle: text_frame(tag, "TIT3"),
        bpm: text_frame(tag, "TBPM").and_then(|s| s.trim().parse::<u32>().ok()),
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
        rating,
        play_count,
        compilation,

        user_text,
        urls,
        extra_text,
    }
}

/// Get a best-effort string value from a frame id.
/// This is intentionally defensive: some frames that are "text-ish"
/// may not be represented as `Content::Text`.
fn text_frame(tag: &Tag, id: &str) -> Option<String> {
    let frame = tag.get(id)?;

    match frame.content() {
        Content::Text(s) => Some(s.clone()),
        Content::Link(s) => Some(s.clone()),
        _ => None,
    }
}

fn first_comment(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() == "COMM" {
            if let Content::Comment(c) = frame.content() {
                return Some(c.text.clone());
            }
        }
    }
    None
}

fn first_lyrics(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() == "USLT" {
            if let Content::Lyrics(l) = frame.content() {
                return Some(l.text.clone());
            }
        }
    }
    None
}

fn collect_user_text(tag: &Tag) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        if frame.id() == "TXXX" {
            if let Content::ExtendedText(et) = frame.content() {
                out.insert(et.description.clone(), et.value.clone());
            }
        }
    }

    out
}

fn collect_urls(tag: &Tag) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        let id = frame.id();
        if !id.starts_with('W') {
            continue;
        }

        match frame.content() {
            Content::Link(url) => {
                out.insert(id.to_string(), url.clone());
            }
            Content::ExtendedLink(el) => {
                let key = format!("WXXX:{}", el.description);
                out.insert(key, el.link.clone());
            }
            _ => {}
        }
    }

    out
}

fn popm_rating_and_count(tag: &Tag) -> (Option<u8>, Option<u64>) {
    for frame in tag.frames() {
        if frame.id() == "POPM" {
            if let Content::Popularimeter(p) = frame.content() {
                return (Some(p.rating), Some(p.counter));
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

        let unk = frame.content().to_unknown().ok()?;
        return parse_be_u64(unk.as_ref().data.as_slice());
    }

    None
}

fn collect_extra_text(tag: &Tag) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        let id = frame.id();

        if !id.starts_with('T') || is_known_text_frame(id) {
            continue;
        }

        if let Content::Text(s) = frame.content() {
            out.insert(id.to_string(), s.clone());
        }
    }

    out
}

fn is_known_text_frame(id: &str) -> bool {
    matches!(
        id,
        "TIT2"
            | "TPE1"
            | "TALB"
            | "TPE2"
            | "TRCK"
            | "TPOS"
            | "TYER"
            | "TDRC"
            | "TCON"
            | "TCOM"
            | "TEXT"
            | "TPE3"
            | "TPE4"
            | "TPUB"
            | "TIT1"
            | "TIT3"
            | "TBPM"
            | "TKEY"
            | "TMOO"
            | "TLAN"
            | "TSRC"
            | "TSSE"
            | "TENC"
            | "TCOP"
            | "TSOT"
            | "TSOP"
            | "TSOA"
            | "TSO2"
            | "TLEN"
            | "TCMP"
            | "TXXX"
            | "COMM"
            | "USLT"
            | "POPM"
            | "PCNT"
            | "APIC"
            | "PIC"
    )
}
