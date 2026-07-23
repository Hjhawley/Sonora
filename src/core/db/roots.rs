//! core/db/roots.rs
//!
//! Persistent library-root configuration and root-related cleanup.
//!
//! Removing a root and deleting tracks that are no longer covered by any
//! enabled root happen in one SQLite transaction.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};

use super::Db;
use super::path_to_db_text;
use crate::core::types::TrackId;

impl Db {
    pub fn load_roots(&self) -> Result<Vec<PathBuf>, String> {
        load_enabled_roots(&self.conn)
    }

    pub fn add_root(&self, root: &Path) -> Result<(), String> {
        let root_text = path_to_db_text(root)?;

        self.conn
            .execute(
                r#"
                INSERT INTO library_roots(path, enabled)
                VALUES (?1, 1)
                ON CONFLICT(path) DO UPDATE SET enabled = 1
                "#,
                params![root_text],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Remove a configured root and atomically delete only the tracks that:
    /// - are beneath the removed root, and
    /// - are not beneath any remaining enabled root.
    ///
    /// The signature remains 'Result<(), String>' so existing callers do not
    /// need to consume a cleanup count.
    pub fn remove_root(&self, root: &Path) -> Result<(), String> {
        let root_text = path_to_db_text(root)?;
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;

        let removed = tx
            .execute(
                "DELETE FROM library_roots WHERE path = ?1",
                params![root_text],
            )
            .map_err(|e| e.to_string())?;

        if removed > 0 {
            let remaining_roots = load_enabled_roots(&tx)?;

            delete_uncovered_tracks(&tx, root, &remaining_roots)?;
        }

        tx.commit().map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Compatibility helper for older callers that explicitly perform cleanup
    /// after removing a root.
    ///
    /// 'remove_root' now performs this cleanup atomically itself, so new code
    /// should not need to call this method. Calling it afterward is harmless
    /// and will ordinarily return zero.
    pub fn delete_uncovered_tracks_under_root(
        &self,
        removed_root: &Path,
        remaining_roots: &[PathBuf],
    ) -> Result<usize, String> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;

        let deleted = delete_uncovered_tracks(&tx, removed_root, remaining_roots)?;

        tx.commit().map_err(|e| e.to_string())?;

        Ok(deleted)
    }
}

fn load_enabled_roots(conn: &Connection) -> Result<Vec<PathBuf>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT path
            FROM library_roots
            WHERE enabled = 1
            ORDER BY path
            "#,
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let path: String = row.get(0)?;
            Ok(PathBuf::from(path))
        })
        .map_err(|e| e.to_string())?;

    let mut roots = Vec::new();

    for row in rows {
        roots.push(row.map_err(|e| e.to_string())?);
    }

    Ok(roots)
}

fn delete_uncovered_tracks(
    conn: &Connection,
    removed_root: &Path,
    remaining_roots: &[PathBuf],
) -> Result<usize, String> {
    let mut ids_to_delete = Vec::new();

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

            if !path.starts_with(removed_root) {
                continue;
            }

            let still_covered = remaining_roots.iter().any(|root| path.starts_with(root));

            if !still_covered {
                ids_to_delete.push(id);
            }
        }
    }

    if ids_to_delete.is_empty() {
        return Ok(0);
    }

    let mut delete = conn
        .prepare("DELETE FROM tracks WHERE id = ?1")
        .map_err(|e| e.to_string())?;

    for id in &ids_to_delete {
        delete.execute(params![id.0]).map_err(|e| e.to_string())?;
    }

    Ok(ids_to_delete.len())
}
