//! ID3 tag reading utilities.
//!
//! This module turns an MP3 file path into a `TrackRow`.
//! We use the `id3` crate to read tags.
//!
//! API design:
//! - `read_track_row(path)` returns `(TrackRow, bool)`
//!   - bool = true means "tag read failed"
//!   - bool = false means "tag read succeeded"

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use id3::Version;
use id3::frame::Content;
use id3::frame::{Comment, Lyrics};
use id3::{Tag, TagLike};

use super::types::TrackRow;

/// Read metadata from a single MP3 file and convert it into a `TrackRow`.
///
/// Why does it take `PathBuf` (owned) instead of `&Path` (borrowed)?
/// - Because `TrackRow` stores the path.
/// - It's convenient to move the `PathBuf` into `TrackRow` without cloning.
///
/// Returns:
/// - `(TrackRow, false)` if tags were read successfully
/// - `(TrackRow, true)` if tag reading failed (TrackRow will have None metadata)
pub fn read_track_row(path: PathBuf) -> (TrackRow, bool) {
    match Tag::read_from_path(&path) {
        Ok(tag) => (build_row_from_tag(path, &tag), false),
        Err(_) => (empty_row(path), true),
    }
}

/// Build a fully-populated TrackRow from an id3::Tag.
///
/// This is separated into a helper so the happy-path stays readable.
fn build_row_from_tag(path: PathBuf, tag: &Tag) -> TrackRow {
    // Pull TRCK / TPOS string values so we can parse totals (e.g. "3/12").
    let (track_no_from_text, track_total) =
        parse_slash_pair_u32(text_frame(tag, "TRCK").as_deref());
    let (disc_no_from_text, disc_total) = parse_slash_pair_u32(text_frame(tag, "TPOS").as_deref());

    // Prefer the crate helper for the main number if it exists.
    // (It usually reads TRCK/TPOS under the hood.)
    let track_no = tag.track().or(track_no_from_text);
    let disc_no = tag.disc().or(disc_no_from_text);

    // Date: most modern tags use TDRC (full date).
    // Year: TagLike::year() best-effort parses.
    let date = text_frame(tag, "TDRC").or_else(|| text_frame(tag, "TYER"));
    let year = tag.year();

    // Artwork count: count APIC (v2.3/2.4) and PIC (old v2.2) frames.
    let artwork_count = tag
        .frames()
        .filter(|f| f.id() == "APIC" || f.id() == "PIC")
        .count() as u32;

    // Pull “big text” frames
    let comment = first_comment(tag);
    let lyrics = first_lyrics(tag);

    // User-defined text (TXXX) and URLs (W* frames).
    let user_text = collect_user_text(tag);
    let urls = collect_urls(tag);

    // Compilation flag is messy in real libraries.
    // We'll try a couple common places.
    let compilation = text_frame(tag, "TCMP")
        .and_then(|s| parse_boolish(&s))
        .or_else(|| user_text.get("COMPILATION").and_then(|s| parse_boolish(s)));

    // Rating / play count:
    // - POPM is common (iTunes etc)
    // - PCNT exists too
    let (rating, popm_count) = popm_rating_and_count(tag);
    let pcnt_count = pcnt_count(tag);
    let play_count = popm_count.or(pcnt_count);

    // Duration (TLEN) is optional and unreliable but easy to capture.
    let duration_ms = text_frame(tag, "TLEN").and_then(|s| s.trim().parse::<u32>().ok());

    // Collect extra text frames as an "escape hatch".
    // We exclude ones we already mapped to explicit struct fields.
    let extra_text = collect_extra_text(tag);

    TrackRow {
        path,

        // Core tags
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

        // Common extended tags
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

        // Sort tags
        title_sort: text_frame(tag, "TSOT"),
        artist_sort: text_frame(tag, "TSOP"),
        album_sort: text_frame(tag, "TSOA"),
        album_artist_sort: text_frame(tag, "TSO2"),

        // Stats / library
        duration_ms,
        rating,
        play_count,
        compilation,

        // Escape hatches
        user_text,
        urls,
        extra_text,
    }
}

/// Build an "empty metadata" row when tag reading fails.
fn empty_row(path: PathBuf) -> TrackRow {
    TrackRow {
        path,

        title: None,
        artist: None,
        album: None,
        album_artist: None,
        composer: None,

        track_no: None,
        track_total: None,
        disc_no: None,
        disc_total: None,

        year: None,
        date: None,
        genre: None,

        grouping: None,
        comment: None,
        lyrics: None,
        lyricist: None,
        conductor: None,
        remixer: None,
        publisher: None,
        subtitle: None,
        bpm: None,
        key: None,
        mood: None,
        language: None,
        isrc: None,
        encoder_settings: None,
        encoded_by: None,
        copyright: None,

        artwork_count: 0,

        title_sort: None,
        artist_sort: None,
        album_sort: None,
        album_artist_sort: None,

        duration_ms: None,
        rating: None,
        play_count: None,
        compilation: None,

        user_text: BTreeMap::new(),
        urls: BTreeMap::new(),
        extra_text: BTreeMap::new(),
    }
}

/// Get the first "plain text" value from a specific ID3 frame id.
/// Examples: "TPE2", "TCOM", "TBPM", ...
fn text_frame(tag: &Tag, id: &str) -> Option<String> {
    let frame = tag.get(id)?;
    match frame.content() {
        Content::Text(s) => Some(s.clone()),
        // TXXX uses ExtendedText; we handle it elsewhere in collect_user_text().
        _ => None,
    }
}

/// Parse strings like:
/// - "3" -> (Some(3), None)
/// - "3/12" -> (Some(3), Some(12))
fn parse_slash_pair_u32(s: Option<&str>) -> (Option<u32>, Option<u32>) {
    let Some(s) = s else { return (None, None) };
    let s = s.trim();
    if s.is_empty() {
        return (None, None);
    }

    let mut parts = s.split('/');
    let a = parts.next().and_then(|p| p.trim().parse::<u32>().ok());
    let b = parts.next().and_then(|p| p.trim().parse::<u32>().ok());
    (a, b)
}

/// Parse common "boolean-ish" tag values.
/// Accepts: "1", "0", "true", "false", "yes", "no", "y", "n"
fn parse_boolish(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Find the first COMM frame and return its text.
fn first_comment(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() != "COMM" {
            continue;
        }
        if let Content::Comment(c) = frame.content() {
            return Some(c.text.clone());
        }
    }
    None
}

/// Find the first USLT frame and return its text.
fn first_lyrics(tag: &Tag) -> Option<String> {
    for frame in tag.frames() {
        if frame.id() != "USLT" {
            continue;
        }
        if let Content::Lyrics(l) = frame.content() {
            return Some(l.text.clone());
        }
    }
    None
}

/// Collect all TXXX user-defined text frames into a map:
/// description -> value
fn collect_user_text(tag: &Tag) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        if frame.id() != "TXXX" {
            continue;
        }
        if let Content::ExtendedText(et) = frame.content() {
            out.insert(et.description.clone(), et.value.clone());
        }
    }

    out
}

/// Collect URL frames.
/// - Most W*** frames are "just a URL"
/// - WXXX is "description + URL"
fn collect_urls(tag: &Tag) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        let id = frame.id();

        // URL frames start with 'W' in ID3.
        if !id.starts_with('W') {
            continue;
        }

        match frame.content() {
            Content::Link(url) => {
                out.insert(id.to_string(), url.clone());
            }
            Content::ExtendedLink(el) => {
                // Store as "WXXX:<description>" so multiple can coexist.
                let key = format!("WXXX:{}", el.description);
                out.insert(key, el.link.clone());
            }
            _ => {}
        }
    }

    out
}

/// Extract rating + play count from POPM frame if present.
///
/// POPM can appear multiple times (different emails).
/// We'll take the *first* one we find for MVP.
fn popm_rating_and_count(tag: &Tag) -> (Option<u8>, Option<u64>) {
    for frame in tag.frames() {
        if frame.id() != "POPM" {
            continue;
        }
        if let Content::Popularimeter(p) = frame.content() {
            return (Some(p.rating), Some(p.counter));
        }
    }
    (None, None)
}

/// Extract play count from PCNT if present.
///
/// PCNT is a binary frame (variable-length big-endian integer).
/// Many versions of the `id3` crate expose it as `Content::Unknown(Vec<u8>)`,
/// not as a dedicated `Content::Counter` variant.
fn pcnt_count(tag: &Tag) -> Option<u64> {
    for frame in tag.frames() {
        if frame.id() != "PCNT" {
            continue;
        }

        // Future-proof: even if id3 adds a dedicated PCNT variant later,
        // this still gives you an Unknown view of the raw bytes.
        let unk = frame.content().to_unknown().ok()?;
        return parse_be_u64(unk.as_ref().data.as_slice());
    }
    None
}

/// Parse a variable-length big-endian integer into u64 (ID3 PCNT format).
fn parse_be_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }

    // If it's longer than 8 bytes, keep the least-significant 8.
    let bytes = if bytes.len() > 8 {
        &bytes[bytes.len() - 8..]
    } else {
        bytes
    };

    let mut v: u64 = 0;
    for &b in bytes {
        v = (v << 8) | (b as u64);
    }
    Some(v)
}

/// Collect "extra" text frames we didn't explicitly model as fields.
/// This lets us show advanced tags in the UI.
fn collect_extra_text(tag: &Tag) -> BTreeMap<String, String> {
    // Frame IDs we already store as explicit fields. Everything else can go to extra_text.
    let known: HashSet<&'static str> = HashSet::from([
        "TIT2", "TPE1", "TALB", "TPE2", "TRCK", "TPOS", "TYER", "TDRC", "TCON", "TCOM", "TEXT",
        "TPE3", "TPE4", "TPUB", "TIT1", "TIT3", "TBPM", "TKEY", "TMOO", "TLAN", "TSRC", "TSSE",
        "TENC", "TCOP", "TSOT", "TSOP", "TSOA", "TSO2", "TLEN", "TCMP",
        // not "text", but we also special-handle these elsewhere:
        "TXXX", "COMM", "USLT", "POPM", "PCNT", "APIC", "PIC",
    ]);

    let mut out = BTreeMap::new();

    for frame in tag.frames() {
        let id = frame.id();

        // Only collect text frames (T***), and skip ones we already mapped.
        if !id.starts_with('T') || known.contains(id) {
            continue;
        }

        if let Content::Text(s) = frame.content() {
            out.insert(id.to_string(), s.clone());
        }
    }

    out
}

// Helper: set/remove a plain text frame (T***)
fn set_text_opt(tag: &mut Tag, id: &str, v: &Option<String>) {
    match v.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => tag.set_text(id, s.to_string()),
        _ => {
            tag.remove(id);
        }
    }
}

// Helper: write TRCK/TPOS as "n" or "n/total" (or remove if None)
fn set_slash_pair(tag: &mut Tag, id: &str, n: Option<u32>, total: Option<u32>) {
    match n {
        None => {
            let _ = tag.remove(id); // TagLike::remove returns Vec<Frame>; discard it
        }
        Some(n) => match total {
            Some(t) => tag.set_text(id, format!("{}/{}", n, t)),
            None => tag.set_text(id, n.to_string()),
        },
    }
}

/// Write tags for a single file, based on the desired contents of `row`.
/// - Always writes "core" fields.
/// - Writes "extended" fields only if `write_extended == true`.
///
/// Semantics:
/// - `None` (or empty/whitespace string) => remove that frame from the file.
pub fn write_track_row(row: &TrackRow, write_extended: bool) -> Result<(), String> {
    let path = &row.path;

    // Load existing tag if possible; otherwise start fresh.
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());

    set_text_opt(&mut tag, "TIT2", &row.title); // title
    set_text_opt(&mut tag, "TPE1", &row.artist); // artist
    set_text_opt(&mut tag, "TALB", &row.album); // album
    set_text_opt(&mut tag, "TPE2", &row.album_artist); // album artist
    set_text_opt(&mut tag, "TCOM", &row.composer); // composer
    set_text_opt(&mut tag, "TCON", &row.genre); // genre

    // Track/disc (use standard TRCK/TPOS formatting)
    set_slash_pair(&mut tag, "TRCK", row.track_no, row.track_total);
    set_slash_pair(&mut tag, "TPOS", row.disc_no, row.disc_total);

    // Year (best-effort)
    match row.year {
        Some(y) => tag.set_year(y),
        None => tag.remove_year(),
    }

    // Date string (keep as text; real libraries vary)
    set_text_opt(&mut tag, "TDRC", &row.date);

    // extended fields
    if write_extended {
        set_text_opt(&mut tag, "TIT1", &row.grouping);
        // Comment (COMM): replace with a single "eng" comment
        match row.comment.as_deref().map(str::trim) {
            Some(s) if !s.is_empty() => {
                let _ = tag.remove("COMM");
                let _ = tag.add_frame(Comment {
                    lang: "eng".to_string(),
                    description: "".to_string(),
                    text: s.to_string(),
                });
            }
            _ => {
                let _ = tag.remove("COMM");
            }
        }
        // Lyrics (USLT): replace with a single "eng" lyrics frame for MVP
        match row.lyrics.as_deref().map(str::trim) {
            Some(s) if !s.is_empty() => {
                let _ = tag.remove("USLT");
                let _ = tag.add_frame(Lyrics {
                    lang: "eng".to_string(),
                    description: "".to_string(),
                    text: s.to_string(),
                });
            }
            _ => {
                let _ = tag.remove("USLT");
            }
        }
        set_text_opt(&mut tag, "TEXT", &row.lyricist);
        set_text_opt(&mut tag, "TPE3", &row.conductor);
        set_text_opt(&mut tag, "TPE4", &row.remixer);
        set_text_opt(&mut tag, "TPUB", &row.publisher);
        set_text_opt(&mut tag, "TIT3", &row.subtitle);

        match row.bpm {
            Some(b) => tag.set_text("TBPM", b.to_string()),
            None => {
                let _ = tag.remove("TBPM"); // discard Vec<Frame>
            }
        }

        set_text_opt(&mut tag, "TKEY", &row.key);
        set_text_opt(&mut tag, "TMOO", &row.mood);
        set_text_opt(&mut tag, "TLAN", &row.language);
        set_text_opt(&mut tag, "TSRC", &row.isrc);
        set_text_opt(&mut tag, "TSSE", &row.encoder_settings);
        set_text_opt(&mut tag, "TENC", &row.encoded_by);
        set_text_opt(&mut tag, "TCOP", &row.copyright);
    }

    // Write back to file (choose v2.4 consistently for now)
    tag.write_to_path(path, Version::Id3v24)
        .map_err(|e| format!("write_to_path failed: {e}"))?;

    Ok(())
}
