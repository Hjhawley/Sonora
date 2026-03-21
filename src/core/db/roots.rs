//! core/db/roots.rs
//!
//! Persistent library-root configuration and root-related cleanup.

use std::path::{Path, PathBuf};

use rusqlite::params;

use super::Db;
use crate::core::types::TrackId;

impl Db {
    pub fn load_roots(&self) -> Result<Vec<PathBuf>, String> {
        let mut stmt = self
            .conn
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

        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| e.to_string())?);
        }
        Ok(out)
    }

    pub fn add_root(&self, root: &Path) -> Result<(), String> {
        let root_str = root.to_string_lossy();

        self.conn
            .execute(
                r#"
                INSERT INTO library_roots(path, enabled)
                VALUES (?1, 1)
                ON CONFLICT(path) DO UPDATE SET enabled = 1
                "#,
                params![root_str.as_ref()],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn remove_root(&self, root: &Path) -> Result<(), String> {
        let root_str = root.to_string_lossy();

        self.conn
            .execute(
                "DELETE FROM library_roots WHERE path = ?1",
                params![root_str.as_ref()],
            )
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn delete_uncovered_tracks_under_root(
        &self,
        removed_root: &Path,
        remaining_roots: &[PathBuf],
    ) -> Result<usize, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM tracks")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((TrackId(id), PathBuf::from(path)))
            })
            .map_err(|e| e.to_string())?;

        let mut ids_to_delete: Vec<TrackId> = Vec::new();

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

        if ids_to_delete.is_empty() {
            return Ok(0);
        }

        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        {
            let mut delete = tx
                .prepare("DELETE FROM tracks WHERE id = ?1")
                .map_err(|e| e.to_string())?;

            for id in &ids_to_delete {
                delete.execute(params![id.0]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;

        Ok(ids_to_delete.len())
    }
}
