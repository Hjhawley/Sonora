//! core/tags/write.rs
//! Write selected ID3 tags back to an MP3, based on a `TrackRow`.

use id3::frame::{Comment, Lyrics};
use id3::{Tag, TagLike, Version};

use super::super::types::TrackRow;

/// Helper: remove all frames with a given id.
/// (TagLike::remove returns Vec<Frame>; discard it.)
fn remove_all(tag: &mut Tag, id: &str) {
    let _ = tag.remove(id);
}

/// Helper: set/remove a plain text frame (T***).
/// - Some(s) where s is non-empty => set_text
/// - None / empty => remove that id
fn set_text_opt(tag: &mut Tag, id: &str, v: &Option<String>) {
    match v.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            // Ensure we don't accumulate duplicates in weird tag states.
            remove_all(tag, id);
            tag.set_text(id, s.to_string());
        }
        _ => remove_all(tag, id),
    }
}

/// Helper: write TRCK/TPOS as "n" or "n/total" (or remove if None)
fn set_slash_pair(tag: &mut Tag, id: &str, n: Option<u32>, total: Option<u32>) {
    match n {
        None => remove_all(tag, id),
        Some(n) => {
            remove_all(tag, id);
            match total {
                Some(t) => tag.set_text(id, format!("{n}/{t}")),
                None => tag.set_text(id, n.to_string()),
            }
        }
    }
}

/// Helper: replace with a single COMM (eng, empty desc) or remove all COMM if empty/None
fn set_comment_opt(tag: &mut Tag, v: &Option<String>) {
    match v.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            remove_all(tag, "COMM");
            // id3 crate supports adding Comment directly (your code already assumes this).
            tag.add_frame(Comment {
                lang: "eng".to_string(),
                description: "".to_string(),
                text: s.to_string(),
            });
        }
        _ => remove_all(tag, "COMM"),
    }
}

/// Helper: replace with a single USLT (eng, empty desc) or remove all USLT if empty/None
fn set_lyrics_opt(tag: &mut Tag, v: &Option<String>) {
    match v.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            remove_all(tag, "USLT");
            tag.add_frame(Lyrics {
                lang: "eng".to_string(),
                description: "".to_string(),
                text: s.to_string(),
            });
        }
        _ => remove_all(tag, "USLT"),
    }
}

/// Write tags for a single file, based on the desired contents of `row`.
/// - Always writes "standard" fields (visible by default in UI).
/// - Writes "extended" fields only if `write_extended == true`.
///
/// Semantics:
/// - `None` (or empty/whitespace string) => remove that frame from the file.
pub fn write_track_row(row: &TrackRow, write_extended: bool) -> Result<(), String> {
    let path = &row.path;

    // Load existing tag if possible; otherwise start fresh.
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());

    // -------------------------
    // Standard (always written)
    // -------------------------
    set_text_opt(&mut tag, "TIT2", &row.title); // title
    set_text_opt(&mut tag, "TPE1", &row.artist); // artist
    set_text_opt(&mut tag, "TALB", &row.album); // album
    set_text_opt(&mut tag, "TPE2", &row.album_artist); // album artist
    set_text_opt(&mut tag, "TCOM", &row.composer); // composer
    set_text_opt(&mut tag, "TCON", &row.genre); // genre

    // Track/disc (use standard TRCK/TPOS formatting)
    set_slash_pair(&mut tag, "TRCK", row.track_no, row.track_total);
    set_slash_pair(&mut tag, "TPOS", row.disc_no, row.disc_total);

    // Year: write via helper AND mirror to TYER for compatibility (some tools still expect it).
    match row.year {
        Some(y) => {
            tag.set_year(y);
            // Mirror:
            remove_all(&mut tag, "TYER");
            tag.set_text("TYER", y.to_string());
        }
        None => {
            tag.remove_year();
            remove_all(&mut tag, "TYER");
        }
    }

    // These are "standard" in your UI (good call keeping them always writable).
    set_text_opt(&mut tag, "TIT1", &row.grouping); // grouping
    set_comment_opt(&mut tag, &row.comment); // comment
    set_lyrics_opt(&mut tag, &row.lyrics); // lyrics
    set_text_opt(&mut tag, "TEXT", &row.lyricist); // lyricist

    // -------------------------
    // Extended (toggleable)
    // -------------------------
    if write_extended {
        // Date string: use TDRC (v2.4-friendly), but also mirror to TYER if year is None
        // and the date begins with "YYYY".
        set_text_opt(&mut tag, "TDRC", &row.date);

        // If user typed a date like "1999-05-14" and year wasn't explicitly set,
        // ensure year() stays consistent for older players.
        if row.year.is_none() {
            if let Some(d) = row.date.as_deref().map(str::trim) {
                if d.len() >= 4 {
                    if let Ok(y) = d[0..4].parse::<i32>() {
                        tag.set_year(y);
                        remove_all(&mut tag, "TYER");
                        tag.set_text("TYER", y.to_string());
                    }
                }
            }
        }

        set_text_opt(&mut tag, "TPE3", &row.conductor);
        set_text_opt(&mut tag, "TPE4", &row.remixer);
        set_text_opt(&mut tag, "TPUB", &row.publisher);
        set_text_opt(&mut tag, "TIT3", &row.subtitle);

        match row.bpm {
            Some(b) => {
                remove_all(&mut tag, "TBPM");
                tag.set_text("TBPM", b.to_string());
            }
            None => remove_all(&mut tag, "TBPM"),
        }

        set_text_opt(&mut tag, "TKEY", &row.key);
        set_text_opt(&mut tag, "TMOO", &row.mood);
        set_text_opt(&mut tag, "TLAN", &row.language);
        set_text_opt(&mut tag, "TSRC", &row.isrc);
        set_text_opt(&mut tag, "TSSE", &row.encoder_settings);
        set_text_opt(&mut tag, "TENC", &row.encoded_by);
        set_text_opt(&mut tag, "TCOP", &row.copyright);
    }

    // Write back to file:
    // - Prefer v2.4 (modern frames like TDRC).
    // - If that fails for some reason, fall back to v2.3.
    if let Err(e) = tag.write_to_path(path, Version::Id3v24) {
        tag.write_to_path(path, Version::Id3v23)
            .map_err(|e2| format!("write_to_path failed: v2.4={e} ; v2.3={e2}"))?;
    }

    Ok(())
}
