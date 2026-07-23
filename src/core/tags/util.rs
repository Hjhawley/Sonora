//! core/tags/util.rs
//!
//! Small parsing and normalization helpers shared by tag reading and writing.

/// Parse common boolean-shaped tags.
///
/// Accepted true values:
/// - `"1"`
/// - `"true"`
/// - `"yes"`
/// - `"y"`
///
/// Accepted false values:
/// - `"0"`
/// - `"false"`
/// - `"no"`
/// - `"n"`
pub(crate) fn parse_boolish(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Parse a variable-length big-endian integer into `u64`.
///
/// ID3 `PCNT` frames use this representation. Values longer than eight bytes
/// retain their least-significant eight bytes.
pub(crate) fn parse_be_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() {
        return None;
    }

    let significant_bytes = if bytes.len() > 8 {
        &bytes[bytes.len() - 8..]
    } else {
        bytes
    };

    let mut value: u64 = 0;

    for &byte in significant_bytes {
        value = (value << 8) | u64::from(byte);
    }

    Some(value)
}

/// Normalize a raw release-date-like string into Sonora's canonical shape.
///
/// Output is always one of:
/// - `YYYY`
/// - `YYYY-MM-DD`
///
/// Reading is intentionally permissive:
/// - `1997` becomes `1997`
/// - `1997-09-29` becomes `1997-09-29`
/// - `1997-09-29T00:00:00` becomes `1997-09-29`
/// - `1997-09-29 00:00:00` becomes `1997-09-29`
/// - `1997-09` becomes `1997`
pub(crate) fn normalize_release_date(raw: &str) -> Option<String> {
    let value = raw.trim();

    if value.is_empty() {
        return None;
    }

    if let Some(prefix) = value.get(0..10) {
        if is_valid_release_date_yyyy_mm_dd(prefix) {
            return Some(prefix.to_string());
        }
    }

    if let Some(prefix) = value.get(0..4) {
        if is_valid_release_date_yyyy(prefix) {
            return Some(prefix.to_string());
        }
    }

    None
}

/// Extract the four-digit year prefix from a canonical or raw release date.
pub(crate) fn extract_year_from_release_date(value: Option<&str>) -> Option<i32> {
    let normalized = normalize_release_date(value?)?;
    normalized.get(0..4)?.parse::<i32>().ok()
}

/// Return whether the input is exactly `YYYY`.
pub(crate) fn is_valid_release_date_yyyy(value: &str) -> bool {
    value.len() == 4 && value.as_bytes().iter().all(u8::is_ascii_digit)
}

/// Return whether the input is exactly `YYYY-MM-DD` with a basic valid range.
///
/// This checks:
/// - month `1..=12`
/// - day `1..=31`
///
/// It intentionally does not validate month-specific day counts or leap years.
pub(crate) fn is_valid_release_date_yyyy_mm_dd(value: &str) -> bool {
    if value.len() != 10 {
        return false;
    }

    let bytes = value.as_bytes();

    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }

    if !bytes[0..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return false;
    }

    let month = match value[5..7].parse::<u32>() {
        Ok(month) => month,
        Err(_) => return false,
    };

    let day = match value[8..10].parse::<u32>() {
        Ok(day) => day,
        Err(_) => return false,
    };

    (1..=12).contains(&month) && (1..=31).contains(&day)
}
