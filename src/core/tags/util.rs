//! core/tags/util.rs
//! Small parsing helpers shared by tag reading/writing.

/// Parse strings like:
/// - "3" -> (Some(3), None)
/// - "3/12" -> (Some(3), Some(12))
pub(crate) fn parse_slash_pair_u32(s: Option<&str>) -> (Option<u32>, Option<u32>) {
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
pub(crate) fn parse_boolish(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Parse a variable-length big-endian integer into u64 (ID3 PCNT format).
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
