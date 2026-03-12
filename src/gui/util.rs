//! gui/util.rs
//! Small pure helper functions used by the GUI.
//! - no UI widgets or state mutation

#![allow(dead_code)]

use std::borrow::Cow;
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

/// Format 'TrackRow' into a compact one-line label for Track View.
pub(crate) fn format_track_one_line(t: &TrackRow) -> String {
    let title: Cow<'_, str> = match t.title.as_deref() {
        Some(s) if !s.trim().is_empty() => Cow::Borrowed(s),
        _ => Cow::Owned(filename_stem(&t.path)),
    };

    let artist: &str = t
        .artist
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Unknown Artist");

    let album: &str = t
        .album
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Unknown Album");

    let track_no: Cow<'_, str> = match t.track_no {
        Some(n) => Cow::Owned(n.to_string()),
        None => Cow::Borrowed("??"),
    };

    format!("#{track_no} — {artist} — {title} ({album})")
}

/// Turn a string into 'Option<String>'.
/// - empty string -> 'None'
/// - non-empty -> 'Some(trimmed_string)'
pub(crate) fn clean_optional_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse an optional u32 from a string.
/// - empty -> 'Ok(None)'
/// - number -> 'Ok(Some(number))'
/// - garbage -> 'Err(())'
pub(crate) fn parse_optional_u32(s: &str) -> Result<Option<u32>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed.parse::<u32>().map(Some).map_err(|_| ())
}

/// Normalize a release date into Sonora's accepted GUI shape:
/// - 'YYYY'
/// - 'YYYY-MM-DD'
pub(crate) fn parse_optional_release_date(s: &str) -> Result<Option<String>, ()> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if is_valid_release_date_yyyy(trimmed) || is_valid_release_date_yyyy_mm_dd(trimmed) {
        return Ok(Some(trimmed.to_string()));
    }

    Err(())
}

/// Extract the 4-digit year prefix from a release date.
pub(crate) fn extract_year_from_release_date(s: Option<&str>) -> Option<i32> {
    let s = s?;
    let year = s.get(0..4)?;
    year.parse::<i32>().ok()
}

fn is_valid_release_date_yyyy(s: &str) -> bool {
    s.len() == 4 && s.as_bytes().iter().all(u8::is_ascii_digit)
}

fn is_valid_release_date_yyyy_mm_dd(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }

    let bytes = s.as_bytes();

    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }

    if !bytes[0..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return false;
    }

    let month = s[5..7].parse::<u32>().map_err(|_| ()).ok();
    let day = s[8..10].parse::<u32>().map_err(|_| ()).ok();

    matches!(month, Some(1..=12)) && matches!(day, Some(1..=31))
}
