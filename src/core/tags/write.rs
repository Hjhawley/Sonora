//! Write selected ID3 tags back to an MP3, based on a `TrackRow`.

use id3::frame::{Comment, Lyrics};
use id3::{Tag, TagLike, Version};

use super::super::types::TrackRow;

/// Helper: set/remove a plain text frame (T***)
fn set_text_opt(tag: &mut Tag, id: &str, v: &Option<String>) {
    match v.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => tag.set_text(id, s.to_string()),
        _ => {
            tag.remove(id);
        }
    }
}

/// Helper: write TRCK/TPOS as "n" or "n/total" (or remove if None)
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
