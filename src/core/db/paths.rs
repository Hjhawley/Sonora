//! core/db/paths.rs
//!
//! Platform-specific database file location helpers.
//!
//! Current policy:
//! - Windows: %LOCALAPPDATA%/Sonora/sonora.sqlite3
//! - macOS:   ~/Library/Application Support/Sonora/sonora.sqlite3
//! - Linux:   $XDG_DATA_HOME/sonora/sonora.sqlite3
//!            or ~/.local/share/sonora/sonora.sqlite3
//!
//! Future expansion could add helpers for cache paths, logs, artwork cache,
//! waveform cache, exports, or backup locations.

use std::path::PathBuf;

pub fn default_db_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA not set".to_string())?;
        let mut dir = PathBuf::from(base);
        dir.push("Sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").ok_or("HOME not set".to_string())?;
        let mut dir = PathBuf::from(home);
        dir.push("Library");
        dir.push("Application Support");
        dir.push("Sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .or_else(|| {
                std::env::var_os("HOME").map(|h| {
                    let mut p = PathBuf::from(h);
                    p.push(".local");
                    p.push("share");
                    p.into_os_string()
                })
            })
            .ok_or("No HOME/XDG_DATA_HOME set".to_string())?;

        let mut dir = PathBuf::from(base);
        dir.push("sonora");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        dir.push("sonora.sqlite3");
        return Ok(dir);
    }
}
