//! core/library.rs
//!
//! Filesystem discovery (read-only).
//!
//! This module is deliberately "dumb":
//! - It ONLY walks folders and returns candidate file paths + lightweight file facts.
//! - It DOES NOT read tags.
//! - It DOES NOT decode audio.
//! - It DOES NOT know about the GUI.
//!
//! This is scan pipeline stage (A): discover paths.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Lightweight filesystem facts discovered during scan.
///
/// These are intentionally cheap to collect and useful for DB-based incremental scans.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub mtime_unix: Option<i64>,
    pub size: Option<u64>,
}

/// Recursively scan a directory tree and return all supported audio files
/// with lightweight filesystem facts.
///
/// Behavior:
/// - Root must be a directory (else Err).
/// - Non-fatal walk errors are skipped (PermissionDenied, NotFound).
/// - Symlinked directories are NOT traversed (prevents cycles).
/// - Symlinked files ARE allowed if they resolve to a file.
/// - Output is sorted by full path.
///
/// Note:
/// Path identity is currently lexical/path-based. A symlinked file and its
/// real path may therefore appear as separate library entries.
pub fn scan_audio_files(root: &Path) -> Result<Vec<DiscoveredFile>, String> {
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let mut out: Vec<DiscoveredFile> = Vec::new();
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

            // Prefer entry.file_type() because it does NOT follow symlinks.
            // This lets us decide whether to traverse directories without
            // accidentally entering symlink cycles.
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
            // We never traverse symlinked directories.
            if ft.is_symlink() {
                match std::fs::metadata(&path) {
                    Ok(md) => {
                        if md.is_file() && is_supported_audio_file(&path) {
                            out.push(discovered_from_metadata(path, &md));
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

            if ft.is_file() && is_supported_audio_file(&path) {
                match entry.metadata() {
                    Ok(md) => out.push(discovered_from_metadata(path, &md)),
                    Err(e) => {
                        if is_nonfatal_walk_error(&e) {
                            continue;
                        }
                        return Err(format!("{}: {e}", path.display()));
                    }
                }
            }
        }
    }

    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn discovered_from_metadata(path: PathBuf, md: &std::fs::Metadata) -> DiscoveredFile {
    let mtime_unix = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);

    let size = Some(md.len());

    DiscoveredFile {
        path,
        mtime_unix,
        size,
    }
}

/// Treat these as "normal" during scans (skip and keep going).
fn is_nonfatal_walk_error(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
    )
}

/// True if the file extension matches a supported audio format.
///
/// Currently only MP3 is supported, but this function intentionally
/// abstracts that decision so additional formats can be added later
/// without changing the scan pipeline.
fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("mp3"))
        .unwrap_or(false)
}
