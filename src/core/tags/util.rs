//! core/tags/util.rs
//!
//! Small parsing / normalization helpers shared by tag reading/writing.
/// Parse strings like:
/// - '"3"' -> '(Some(3), None)'
/// - '"3/12"' -> '(Some(3), Some(12))'
pub(crate) fn parse_slash_pair_u32(s: Option<&str>) -> (Option<u32>, Option<u32>) {
    let Some(s) = s else {
        return (None, None);
    };

    let s = s.trim();
    if s.is_empty() {
        return (None, None);
    }

    let mut parts = s.split('/');
    let a = parts.next().and_then(|p| p.trim().parse::<u32>().ok());
    let b = parts.next().and_then(|p| p.trim().parse::<u32>().ok());
    (a, b)
}

/// Parse common boolean-shaped tags.
/// Accepts: '"1"', '"0"', '"true"', '"false"', '"yes"', '"no"', '"y"', '"n"'
pub(crate) fn parse_boolish(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Parse a variable-length big-endian integer into 'u64' (ID3 PCNT format).
pub(crate) fn parse_be_u64(bytes: &[u8]) -> Option<u64> {
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

/// Normalize a raw release-date-ish string into Sonora's canonical shape.
///
/// Output is always one of:
/// - 'YYYY'
/// - 'YYYY-MM-DD'
///
/// This is intentionally a bit permissive for tag *reading*:
/// - '1997' -> '1997'
/// - '1997-09-29' -> '1997-09-29'
/// - '1997-09-29T00:00:00' -> '1997-09-29'
/// - '1997-09-29 00:00:00' -> '1997-09-29'
/// - '1997-09' -> '1997'
///
/// For *UI input validation* later, you can be stricter if you want.
pub(crate) fn normalize_release_date(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(prefix) = s.get(0..10) {
        if is_valid_release_date_yyyy_mm_dd(prefix) {
            return Some(prefix.to_string());
        }
    }

    if let Some(prefix) = s.get(0..4) {
        if is_valid_release_date_yyyy(prefix) {
            return Some(prefix.to_string());
        }
    }

    None
}

/// Extract the 4-digit year prefix from a canonical or raw release-date string.
pub(crate) fn extract_year_from_release_date(s: Option<&str>) -> Option<i32> {
    let normalized = normalize_release_date(s?)?;
    normalized.get(0..4)?.parse::<i32>().ok()
}

/// True iff the input is exactly 'YYYY'.
pub(crate) fn is_valid_release_date_yyyy(s: &str) -> bool {
    s.len() == 4 && s.as_bytes().iter().all(u8::is_ascii_digit)
}

/// True iff the input is exactly 'YYYY-MM-DD' and month/day are in basic range.
///
/// This does basic calendar sanity only:
/// - month 1..=12
/// - day 1..=31
///
/// It intentionally does not validate month-specific day counts or leap years.
pub(crate) fn is_valid_release_date_yyyy_mm_dd(s: &str) -> bool {
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

    let month = match s[5..7].parse::<u32>() {
        Ok(v) => v,
        Err(_) => return false,
    };

    let day = match s[8..10].parse::<u32>() {
        Ok(v) => v,
        Err(_) => return false,
    };

    (1..=12).contains(&month) && (1..=31).contains(&day)
}
