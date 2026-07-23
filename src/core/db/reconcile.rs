//! core/db/reconcile.rs
//!
//! Scan-time reconciliation between discovered filesystem state and cached
//! SQLite track rows.
//!
//! Reconciliation:
//! - marks tracks under the scanned roots as missing
//! - marks rediscovered files as present
//! - updates cached filesystem facts
//! - identifies new, changed, or stale-cache rows
//! - returns only the rows that must be rehydrated from disk

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};

use super::Db;
use super::path_to_db_text;
use super::schema::TRACK_METADATA_CACHE_VERSION;
use crate::core::library::DiscoveredFile;
use crate::core::types::TrackId;

impl Db {
    /// Reconcile discovered filesystem facts into the database.
    ///
    /// Metadata must be rehydrated when a file is:
    /// - newly discovered
    /// - changed according to its `(mtime, size)` pair
    /// - cached with an older metadata representation
    ///
    /// A failed hydration does not stamp the current cache version, so the
    /// track will be retried during a later scan.
    pub fn upsert_discovered(
        &mut self,
        scanned_roots: &[PathBuf],
        files: &[DiscoveredFile],
    ) -> Result<Vec<(TrackId, PathBuf)>, String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;

        mark_tracks_under_roots_missing(&tx, scanned_roots)?;

        let mut tracks_to_hydrate = Vec::new();

        {
            let mut select_existing = tx
                .prepare(
                    r#"
                    SELECT
                        id,
                        mtime,
                        size,
                        metadata_cache_version
                    FROM tracks
                    WHERE path = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut insert_track = tx
                .prepare(
                    r#"
                    INSERT INTO tracks(
                        path,
                        present,
                        mtime,
                        size,
                        metadata_cache_version
                    )
                    VALUES (?1, 1, ?2, ?3, 0)
                    "#,
                )
                .map_err(|e| e.to_string())?;

            let mut update_filesystem_facts = tx
                .prepare(
                    r#"
                    UPDATE tracks
                    SET
                        present = 1,
                        mtime = ?2,
                        size = ?3
                    WHERE id = ?1
                    "#,
                )
                .map_err(|e| e.to_string())?;

            for file in files {
                let path_text = path_to_db_text(&file.path)?;
                let size = file
                    .size
                    .map(|value| {
                        i64::try_from(value).map_err(|_| {
                            format!(
                                "File size does not fit SQLite INTEGER for {}",
                                file.path.display()
                            )
                        })
                    })
                    .transpose()?;

                let existing: Option<(TrackId, Option<i64>, Option<i64>, i64)> = select_existing
                    .query_row(params![path_text], |row| {
                        Ok((
                            TrackId(row.get::<_, i64>(0)?),
                            row.get::<_, Option<i64>>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    })
                    .optional()
                    .map_err(|e| e.to_string())?;

                match existing {
                    Some((id, old_mtime, old_size, cache_version)) => {
                        update_filesystem_facts
                            .execute(params![id.0, file.mtime_unix, size,])
                            .map_err(|e| e.to_string())?;

                        let needs_refresh = old_mtime != file.mtime_unix
                            || old_size != size
                            || cache_version < TRACK_METADATA_CACHE_VERSION;

                        if needs_refresh {
                            tracks_to_hydrate.push((id, file.path.clone()));
                        }
                    }

                    None => {
                        insert_track
                            .execute(params![path_text, file.mtime_unix, size,])
                            .map_err(|e| e.to_string())?;

                        let id = TrackId(tx.last_insert_rowid());
                        tracks_to_hydrate.push((id, file.path.clone()));
                    }
                }
            }
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok(tracks_to_hydrate)
    }
}

fn mark_tracks_under_roots_missing(
    conn: &rusqlite::Connection,
    scanned_roots: &[PathBuf],
) -> Result<(), String> {
    if scanned_roots.is_empty() {
        return Ok(());
    }

    let mut ids_under_scanned_roots = Vec::new();

    {
        let mut stmt = conn
            .prepare("SELECT id, path FROM tracks")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id = TrackId(row.get::<_, i64>(0)?);
                let path = PathBuf::from(row.get::<_, String>(1)?);
                Ok((id, path))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, path) = row.map_err(|e| e.to_string())?;

            if is_under_any_root(&path, scanned_roots) {
                ids_under_scanned_roots.push(id);
            }
        }
    }

    if ids_under_scanned_roots.is_empty() {
        return Ok(());
    }

    let mut mark_missing = conn
        .prepare("UPDATE tracks SET present = 0 WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    for id in ids_under_scanned_roots {
        mark_missing
            .execute(params![id.0])
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn is_under_any_root(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}
