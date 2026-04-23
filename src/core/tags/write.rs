//! core/tags/write.rs
//!
//! Write selected ID3 tags back to an MP3, based on a 'TrackRow'.

use id3::frame::{Comment, Lyrics};
use id3::{Tag, TagLike, Version};

use super::super::types::TrackRow;
use super::util::normalize_release_date;

/// Remove all frames with a given id.
/// ('TagLike::remove' returns 'Vec<Frame>'; discard it.)
fn remove_all(tag: &mut Tag, id: &str) {
    let _ = tag.remove(id);
}

/// Replace all frames with a single plain text value, or remove them entirely.
/// This is intentionally destructive for duplicate / variant frames sharing
/// the same id.
fn set_or_remove_text_frame(tag: &mut Tag, id: &str, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            remove_all(tag, id);
            tag.set_text(id, s.to_string());
        }
        _ => remove_all(tag, id),
    }
}

/// Write 'TRCK' / 'TPOS' as '"n"' or '"n/total"', or remove if absent.
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

/// Replace all 'COMM' frames with one comment, or remove them if absent.
/// This intentionally collapses multi-language / multi-description comment state
/// into Sonora's single editable comment field.
fn set_comment_opt(tag: &mut Tag, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => {
            remove_all(tag, "COMM");
            tag.add_frame(Comment {
                lang: "eng".to_string(),
                description: "".to_string(),
                text: s.to_string(),
            });
        }
        _ => remove_all(tag, "COMM"),
    }
}

/// Replace all 'USLT' frames with one lyrics frame, or remove them if absent.
/// This intentionally collapses multi-language / multi-description lyrics state
/// into Sonora's single editable lyrics field.
fn set_lyrics_opt(tag: &mut Tag, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
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

fn set_or_remove_numeric_text<T: ToString>(tag: &mut Tag, id: &str, value: Option<T>) {
    match value {
        Some(v) => {
            remove_all(tag, id);
            tag.set_text(id, v.to_string());
        }
        None => remove_all(tag, id),
    }
}

/// Write Sonora's single canonical release-date field.
/// - Simplify to one user-facing date field: 'release_date'
/// - normalized accepted values:
///   - 'YYYY'
///   - 'YYYY-MM-DD'
/// - write only 'TDRC'
/// - clear legacy 'TYER' to avoid duplicate displays like '1994\\1994'
fn write_release_date(tag: &mut Tag, row: &TrackRow) {
    remove_all(tag, "TDRC");
    remove_all(tag, "TYER");

    let normalized = row
        .release_date
        .as_deref()
        .and_then(normalize_release_date)
        .or_else(|| row.year.map(|y| y.to_string()));

    if let Some(value) = normalized {
        tag.set_text("TDRC", value);
    }
}

/// Write tags for a single file, based on the desired contents of 'row'.
/// - 'None' (or empty/whitespace string) => remove that frame from the file.
pub fn write_track_row(row: &TrackRow, _write_extended: bool) -> Result<(), String> {
    let path = &row.path;

    // Load existing tag if possible; otherwise start fresh.
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());

    set_or_remove_text_frame(&mut tag, "TIT2", &row.title);
    set_or_remove_text_frame(&mut tag, "TPE1", &row.artist);
    set_or_remove_text_frame(&mut tag, "TALB", &row.album);
    set_or_remove_text_frame(&mut tag, "TPE2", &row.album_artist);
    set_or_remove_text_frame(&mut tag, "TCOM", &row.composer);
    set_or_remove_text_frame(&mut tag, "TCON", &row.genre);

    set_slash_pair(&mut tag, "TRCK", row.track_no, row.track_total);
    set_slash_pair(&mut tag, "TPOS", row.disc_no, row.disc_total);

    write_release_date(&mut tag, row);

    set_or_remove_text_frame(&mut tag, "TIT1", &row.grouping);
    set_comment_opt(&mut tag, &row.comment);
    set_lyrics_opt(&mut tag, &row.lyrics);
    set_or_remove_text_frame(&mut tag, "TEXT", &row.lyricist);

    set_or_remove_text_frame(&mut tag, "TPE3", &row.conductor);
    set_or_remove_text_frame(&mut tag, "TPE4", &row.remixer);
    set_or_remove_text_frame(&mut tag, "TPUB", &row.publisher);
    set_or_remove_text_frame(&mut tag, "TIT3", &row.subtitle);

    set_or_remove_numeric_text(&mut tag, "TBPM", row.bpm);

    set_or_remove_text_frame(&mut tag, "TKEY", &row.key);
    set_or_remove_text_frame(&mut tag, "TMOO", &row.mood);
    set_or_remove_text_frame(&mut tag, "TLAN", &row.language);
    set_or_remove_text_frame(&mut tag, "TSRC", &row.isrc);
    set_or_remove_text_frame(&mut tag, "TSSE", &row.encoder_settings);
    set_or_remove_text_frame(&mut tag, "TENC", &row.encoded_by);
    set_or_remove_text_frame(&mut tag, "TCOP", &row.copyright);

    // Write back to file, preferring v2.4.
    // If that fails, fall back to v2.3.
    if let Err(e) = tag.write_to_path(path, Version::Id3v24) {
        tag.write_to_path(path, Version::Id3v23)
            .map_err(|e2| format!("write_to_path failed: v2.4={e} ; v2.3={e2}"))?;
    }

    Ok(())
}
