//! core/tags/write.rs
//!
//! Writes Sonora-managed 'TrackRow' metadata back into an MP3 ID3 tag.
//!
//! Sonora replaces or removes the frames it explicitly manages while
//! preserving unrelated existing tag data.

use id3::frame::{Comment, Lyrics};
use id3::{Tag, TagLike, Version};

use super::super::types::TrackRow;
use super::util::normalize_release_date;

const GROUPING_FRAME_ID: &str = "GRP1";
const CONTENT_GROUP_FRAME_ID: &str = "TIT1";

/// Remove all frames with a given identifier.
fn remove_all(tag: &mut Tag, id: &str) {
    let _ = tag.remove(id);
}

/// Replace all frames with one plain-text value, or remove them entirely.
///
/// This intentionally collapses duplicate frames sharing the same identifier.
fn set_or_remove_text_frame(tag: &mut Tag, id: &str, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            remove_all(tag, id);
            tag.set_text(id, value.to_string());
        }
        _ => remove_all(tag, id),
    }
}

/// Write 'TRCK' or 'TPOS' while retaining the user's original digit
/// representation.
///
/// Text components take priority because they preserve formatting such as
/// leading zeroes. Parsed numeric values are used only as a fallback for rows
/// created by code that does not provide textual components.
fn set_counter_pair(
    tag: &mut Tag,
    id: &str,
    number_text: &Option<String>,
    total_text: &Option<String>,
    number: Option<u32>,
    total: Option<u32>,
) {
    let number = counter_component(number_text, number);
    let total = counter_component(total_text, total);

    let Some(number) = number else {
        remove_all(tag, id);
        return;
    };

    remove_all(tag, id);

    match total {
        Some(total) => tag.set_text(id, format!("{number}/{total}")),
        None => tag.set_text(id, number),
    }
}

fn counter_component(text: &Option<String>, numeric: Option<u32>) -> Option<String> {
    match text.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => Some(value.to_string()),
        _ => numeric.map(|value| value.to_string()),
    }
}

/// Replace all 'COMM' frames with one comment, or remove them when absent.
///
/// Sonora intentionally represents comments as one editable value rather than
/// preserving separate language and description variants.
fn set_comment_opt(tag: &mut Tag, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            remove_all(tag, "COMM");

            tag.add_frame(Comment {
                lang: "eng".to_string(),
                description: String::new(),
                text: value.to_string(),
            });
        }

        _ => remove_all(tag, "COMM"),
    }
}

/// Replace all 'USLT' frames with one lyrics frame, or remove them when absent.
///
/// Sonora intentionally represents lyrics as one editable value rather than
/// preserving separate language and description variants.
fn set_lyrics_opt(tag: &mut Tag, value: &Option<String>) {
    match value.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            remove_all(tag, "USLT");

            tag.add_frame(Lyrics {
                lang: "eng".to_string(),
                description: String::new(),
                text: value.to_string(),
            });
        }

        _ => remove_all(tag, "USLT"),
    }
}

fn set_or_remove_numeric_text<T>(tag: &mut Tag, id: &str, value: Option<T>)
where
    T: ToString,
{
    match value {
        Some(value) => {
            remove_all(tag, id);
            tag.set_text(id, value.to_string());
        }

        None => remove_all(tag, id),
    }
}

/// Write Sonora's single canonical release-date field.
///
/// Accepted normalized forms:
/// - 'YYYY'
/// - 'YYYY-MM-DD'
///
/// Sonora writes only 'TDRC' and removes legacy 'TYER' to avoid conflicting
/// duplicate release-date values.
fn write_release_date(tag: &mut Tag, row: &TrackRow) {
    remove_all(tag, "TDRC");
    remove_all(tag, "TYER");

    let normalized = row
        .release_date
        .as_deref()
        .and_then(normalize_release_date)
        .or_else(|| row.year.map(|year| year.to_string()));

    if let Some(value) = normalized {
        tag.set_text("TDRC", value);
    }
}

/// Write Sonora-managed metadata for one file.
/// A missing or whitespace-only string removes the corresponding managed frame.
pub fn write_track_row(row: &TrackRow, _write_extended: bool) -> Result<(), String> {
    let path = &row.path;

    // Preserve existing unrelated frames whenever a readable tag exists.
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::new());

    set_or_remove_text_frame(&mut tag, "TIT2", &row.title);
    set_or_remove_text_frame(&mut tag, "TPE1", &row.artist);
    set_or_remove_text_frame(&mut tag, "TALB", &row.album);
    set_or_remove_text_frame(&mut tag, "TPE2", &row.album_artist);
    set_or_remove_text_frame(&mut tag, "TCOM", &row.composer);
    set_or_remove_text_frame(&mut tag, "TCON", &row.genre);

    set_counter_pair(
        &mut tag,
        "TRCK",
        &row.track_no_text,
        &row.track_total_text,
        row.track_no,
        row.track_total,
    );

    set_counter_pair(
        &mut tag,
        "TPOS",
        &row.disc_no_text,
        &row.disc_total_text,
        row.disc_no,
        row.disc_total,
    );

    write_release_date(&mut tag, row);

    set_or_remove_text_frame(&mut tag, GROUPING_FRAME_ID, &row.grouping);
    set_or_remove_text_frame(&mut tag, CONTENT_GROUP_FRAME_ID, &row.content_group);

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

    if let Err(v24_error) = tag.write_to_path(path, Version::Id3v24) {
        tag.write_to_path(path, Version::Id3v23)
            .map_err(|v23_error| {
                format!(
                    "write_to_path failed: v2.4={v24_error}; \
                     v2.3={v23_error}"
                )
            })?;
    }

    Ok(())
}
