//! Filesystem scanning utilities.
//!
//! Right now we support:
//! - recursive scan
//! - `.mp3` only
//!
//! This module intentionally knows nothing about Iced (GUI) or ID3 tags.
//! It only returns file paths.

use std::path::{Path, PathBuf};

/// Recursively scan a directory tree and return all `.mp3` file paths.
///
/// This does a "walk" using `std::fs::read_dir` and recurses into subfolders.
///
/// Returns an error string if the root can’t be read or a folder can’t be traversed.
/// (Example: permissions, missing directory, etc.)
pub fn scan_mp3s(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    walk_dir(root, &mut out)?;
    Ok(out)
}

/// Recursive helper: walks one directory and pushes matching files into `out`.
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subfolders
            walk_dir(&path, out)?;
        } else if is_mp3(&path) {
            // Found a track file
            out.push(path);
        }
    }

    Ok(())
}

/// True if the file extension is `.mp3` (case-insensitive).
fn is_mp3(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}
