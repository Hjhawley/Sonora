//! Small pure helper functions used by the GUI.
//! - no UI widgets or state mutation

use std::path::Path;

use crate::core::types::TrackRow;

/// Gets filename without extension, used as a fallback title.
/// Ex: 'song.mp3' -> 'song'
pub(crate) fn filename_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Title")
        .to_string()
}

/// Format TrackRow into a compact one-line label for Track View.
pub(crate) fn format_track_one_line(t: &TrackRow) -> String {
    let title = t.title.clone().unwrap_or_else(|| filename_stem(&t.path));
    let artist = t
        .artist
        .clone()
        .unwrap_or_else(|| "Unknown Artist".to_string());
    let album = t
        .album
        .clone()
        .unwrap_or_else(|| "Unknown Album".to_string());

    let track_no = t
        .track_no
        .map(|n| n.to_string())
        .unwrap_or_else(|| "??".to_string());

    format!("#{track_no} — {artist} — {title} ({album})")
}

/// Turn a string into Option<String>.
/// - empty string -> None
/// - non-empty -> Some(trimmed_string)
pub(crate) fn clean_optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse an optional u32 from a string.
/// - empty -> Ok(None)
/// - number -> Ok(Some(number))
/// - garbage -> Err(())
pub(crate) fn parse_optional_u32(s: &str) -> Result<Option<u32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<u32>().map(Some).map_err(|_| ())
}

/// Same idea as above, but for years (i32).
pub(crate) fn parse_optional_i32(s: &str) -> Result<Option<i32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<i32>().map(Some).map_err(|_| ())
}
