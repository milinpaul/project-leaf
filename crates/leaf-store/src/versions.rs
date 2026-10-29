//! Which version of each document is on disk, without loading any of them.
//!
//! Search keeps its own copy of every page's text (ADR 0024). It decides what to re-read by
//! comparing these versions with the ones it indexed, so nothing in the editor has to report what
//! changed, and edits saved just before a crash are still picked up on the next start.

use std::collections::HashMap;
use std::str::FromStr;

use leaf_core::PageId;

use crate::{DocKey, Store, StoreError};

/// A document's saved version: compares equal only if nothing was saved in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedVersion {
    /// When it was last saved, in Unix milliseconds. Used for "most recently edited", not for
    /// change detection: two saves can share a millisecond.
    pub saved_at: i64,
    /// The highest update sequence number saved for it, or 0 for a snapshot alone. Sequence numbers
    /// only ever grow, so any later save changes this.
    pub seq: i64,
}

impl Store {
    /// The saved version of every document in the database, in one query.
    ///
    /// A row whose ID is neither `workspace` nor a page ID (which Leaf never writes) is skipped with
    /// a warning rather than failing the whole listing.
    pub fn saved_versions(&self) -> Result<HashMap<DocKey, SavedVersion>, StoreError> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT d.doc_id, max(d.updated_at, coalesce(max(u.created_at), 0)), coalesce(max(u.seq), 0)
             FROM docs d LEFT JOIN doc_updates u ON u.doc_id = d.doc_id
             GROUP BY d.doc_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SavedVersion {
                    saved_at: row.get(1)?,
                    seq: row.get(2)?,
                },
            ))
        })?;
        let mut versions = HashMap::new();
        for row in rows {
            let (id, version) = row?;
            if let Some(key) = parse_key(&id) {
                versions.insert(key, version);
            } else {
                tracing::warn!(doc = %id, "skipped a document with an unknown ID");
            }
        }
        Ok(versions)
    }
}

fn parse_key(id: &str) -> Option<DocKey> {
    if id == DocKey::Workspace.to_string() {
        Some(DocKey::Workspace)
    } else {
        PageId::from_str(id).ok().map(DocKey::Page)
    }
}
