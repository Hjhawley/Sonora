//! Filesystem scanning utilities (read-only)
//! - Walks folders recursively (visits subfolders too)
//! - Collects files that end in '.mp3'
//! - Does NOT read ID3 tags
//! - Does NOT know anything about the GUI (Iced)
//! - Does NOT write or modify files

use std::path::{Path, PathBuf};

/// Recursively scan a directory tree and return all '.mp3' file paths.
/// - 'root: &Path' means "borrow a path" (we don't take ownership, we just look at it)
/// - 'Result<Vec<PathBuf>, String>' means:
///    - Ok(Vec<PathBuf>) = success; here's our list of file paths
///    - Err(String) = failure; here's an error message we can show to the user
///
/// Failure states:
/// - permissions (Windows "Access denied")
/// - folder doesn't exist
/// - removable drive disconnected
pub fn scan_mp3s(root: &Path) -> Result<Vec<PathBuf>, String> {
    // We'll push matches into this Vec as we find them.
    let mut out = Vec::new();

    // 'walk_dir' does the recursive work
    // '?' means:
    // "If walk_dir returns Err(...), stop immediately and return that Err upward."
    walk_dir(root, &mut out)?;

    Ok(out)
}

/// Recursive helper: walks ONE directory and pushes matching files into 'out'.
/// - 'out: &mut Vec<PathBuf>' means:
///   "Here's the same list; please modify it by pushing into it."
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    // read_dir gives an iterator of entries in this folder.
    // This can fail if we can't read the directory.
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;

    for entry in entries {
        // Each 'entry' is a Result<DirEntry, Error> so we unwrap it safely.
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            // Folder → recurse into it (depth-first).
            walk_dir(&path, out)?;
        } else if is_mp3(&path) {
            // File + mp3 extension → keep it.
            out.push(path);
        }
    }

    Ok(())
}

/// True if the file extension is '.mp3' (case-insensitive).
fn is_mp3(path: &Path) -> bool {
    path.extension()
        // extension() returns Option<OsStr> (it might not exist)
        .and_then(|s| s.to_str())
        // to_str() returns Option<&str> (it might not be valid UTF-8)
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        // if anything failed above, default to "false"
        .unwrap_or(false)
}
