//! core/library.rs
//!
//! Filesystem discovery (read-only).
//!
//! This module is deliberately "dumb":
//! - It ONLY walks folders and returns candidate file paths.
//! - It DOES NOT read tags.
//! - It DOES NOT decode audio.
//! - It DOES NOT know about the GUI.
//! - This is scan pipeline stage (A): discover paths.

use std::path::{Path, PathBuf};

/// Recursively scan a directory tree and return all `.mp3` file paths.
///
/// Behavior:
/// - Root must be a directory (else Err).
/// - Non-fatal walk errors are skipped (PermissionDenied, NotFound).
/// - Symlinked directories are NOT traversed (prevents cycles).
/// - Symlinked files ARE allowed if they ultimately resolve to a file.
/// - Output is sorted by full path.
pub fn scan_mp3s(root: &Path) -> Result<Vec<PathBuf>, String> {
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let mut out: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries: std::fs::ReadDir = match std::fs::read_dir(&dir) {
            Ok(it) => it,
            Err(e) => {
                if is_nonfatal_walk_error(&e) {
                    continue;
                }
                return Err(format!("{}: {e}", dir.display()));
            }
        };

        for entry_res in entries {
            let entry: std::fs::DirEntry = match entry_res {
                Ok(e) => e,
                Err(e) => {
                    if is_nonfatal_walk_error(&e) {
                        continue;
                    }
                    return Err(format!("{}: {e}", dir.display()));
                }
            };

            let path: PathBuf = entry.path();

            // Prefer entry.file_type(): does not follow symlinks.
            let ft: std::fs::FileType = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    if is_nonfatal_walk_error(&e) {
                        continue;
                    }
                    return Err(format!("{}: {e}", path.display()));
                }
            };

            if ft.is_dir() {
                stack.push(path);
                continue;
            }

            // If it's a symlink, follow it ONLY to decide if it's a file we should include.
            // We do not traverse symlinked directories.
            if ft.is_symlink() {
                match std::fs::metadata(&path) {
                    Ok(md) => {
                        if md.is_file() && is_mp3(&path) {
                            out.push(path);
                        }
                    }
                    Err(e) => {
                        if is_nonfatal_walk_error(&e) {
                            continue;
                        }
                        return Err(format!("{}: {e}", path.display()));
                    }
                }
                continue;
            }

            if ft.is_file() && is_mp3(&path) {
                out.push(path);
            }
        }
    }

    out.sort();
    Ok(out)
}

/// Treat these as "normal" during scans (skip and keep going).
fn is_nonfatal_walk_error(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
    )
}

/// True if the file extension is `.mp3` (case-insensitive).
fn is_mp3(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}
