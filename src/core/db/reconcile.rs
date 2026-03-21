//! core/db/reconcile.rs
//!
//! Scan-time reconciliation between discovered filesystem state and cached
//! SQLite track records.

use std::path::PathBuf;

use rusqlite::{OptionalExtension, params};

use super::Db;
use crate::core::library::DiscoveredFile;
use crate::core::types::TrackId;

impl Db {
    /// Reconcile filesystem facts into DB truth for the scanned roots.
    ///
    /// Returns DB-backed `(TrackId, PathBuf)` items whose metadata should be
    /// re-read from disk because the file is:
    /// - new
    /// - changed by `(mtime, size)`
    /// - or still missing cached metadata in the DB
    pub fn upsert_discovered(
        &mut self,
        scanned_roots: &[PathBuf],
        files: &[DiscoveredFile],
    ) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        let mut ids_under_scanned_roots: Vec<TrackId> = Vec::new();
        {
            let mut stmt = tx
                .prepare("SELECT id, path FROM tracks")
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map([], |row| {
                    let id: i64 = row.get(0)?;
                    let path: String = row.get(1)?;
                    Ok((TrackId(id), PathBuf::from(path)))
                })
                .map_err(|e| e.to_string())?;

            for row in rows {
                let (id, path) = row.map_err(|e| e.to_string())?;
                if scanned_roots.iter().any(|root| path.starts_with(root)) {
                    ids_under_scanned_roots.push(id);
                }
            }
        }

        if !ids_under_scanned_roots.is_empty() {
            let mut mark_missing = tx
                .prepare("UPDATE tracks SET present = 0 WHERE id = ?1")
                .map_err(|e| e.to_string())?;

            for id in &ids_under_scanned_roots {
                mark_missing
                    .execute(params![id.0])
                    .map_err(|e| e.to_string())?;
            }
        }

        let mut changed: Vec<(TrackId, PathBuf)> = Vec::new();

        {
            let mut select_existing = tx
                .prepare(
                    r#"
                    SELECT
                        id,
                        mtime,
                        size,
                        title,
                        artist,
                        album,
                        album_artist,
                        duration_ms
                    FROM tracks
                    WHERE path = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO tracks(path) VALUES (?1)")
                .map_err(|e| e.to_string())?;

            let mut update_presence = tx
                .prepare(
                    r#"
                    UPDATE tracks
                    SET present = 1, mtime = ?2, size = ?3
                    WHERE path = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut select_id = tx
                .prepare("SELECT id FROM tracks WHERE path = ?1")
                .map_err(|e| e.to_string())?;

            for f in files {
                let p_str = f.path.to_string_lossy();
                let new_size_i64 = f.size.map(|s| s as i64);

                let existing: Option<(
                    TrackId,
                    Option<i64>,
                    Option<i64>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<i64>,
                )> = select_existing
                    .query_row(params![p_str.as_ref()], |row| {
                        Ok((
                            TrackId(row.get::<_, i64>(0)?),
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<i64>>(7)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;

                let needs_refresh = match existing {
                    None => true,
                    Some((
                        _id,
                        old_mtime,
                        old_size,
                        title,
                        artist,
                        album,
                        album_artist,
                        duration_ms,
                    )) => {
                        old_mtime != f.mtime_unix
                            || old_size != new_size_i64
                            || metadata_cache_missing(
                                title.as_deref(),
                                artist.as_deref(),
                                album.as_deref(),
                                album_artist.as_deref(),
                                duration_ms,
                            )
                    }
                };

                insert
                    .execute(params![p_str.as_ref()])
                    .map_err(|e| e.to_string())?;

                update_presence
                    .execute(params![p_str.as_ref(), f.mtime_unix, new_size_i64])
                    .map_err(|e| e.to_string())?;

                if needs_refresh {
                    let id = TrackId(
                        select_id
                            .query_row(params![p_str.as_ref()], |row| row.get::<_, i64>(0))
                            .map_err(|e| e.to_string())?,
                    );
                    changed.push((id, f.path.clone()));
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;
        Ok(changed)
    }
}

fn metadata_cache_missing(
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
    album_artist: Option<&str>,
    duration_ms: Option<i64>,
) -> bool {
    title.is_none()
        && artist.is_none()
        && album.is_none()
        && album_artist.is_none()
        && duration_ms.is_none()
}
