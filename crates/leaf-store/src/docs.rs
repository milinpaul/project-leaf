//! Saving and loading documents: a snapshot per document plus the updates saved after it.
//!
//! The first save of a document writes a full snapshot. Every later save appends one small Loro
//! update holding all commits since the previous save, so a save costs about as much as the edit,
//! not the page. Loading reads the snapshot, then the updates in the order they were saved.
//! Folding updates back into a fresh snapshot is compaction (E03-S05).

use std::fmt;

use leaf_core::PageId;
use leaf_model::{ModelError, PageDoc, Workspace};
use rusqlite::{OptionalExtension, params};

use crate::{Store, StoreError};

/// Which document a row belongs to: the workspace (page tree) or one page's blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKey {
    /// The workspace document, stored as `doc_id = 'workspace'`.
    Workspace,
    /// A page's blocks, stored under the page's ID.
    Page(PageId),
}

impl fmt::Display for DocKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace => f.write_str("workspace"),
            Self::Page(page) => page.fmt(f),
        }
    }
}

/// A model document the store can save and load. Implemented for [`Workspace`] and [`PageDoc`],
/// whose methods of the same names explain the details; the store only ever handles bytes.
pub trait Document: Sized {
    /// Rebuilds the document from its snapshot and later updates, in saved order.
    fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError>;
    /// The whole document.
    fn to_bytes(&self) -> Result<Vec<u8>, ModelError>;
    /// Changes not yet on disk, as one update, or `None`.
    fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError>;
    /// Everything is on disk now.
    fn mark_saved(&mut self);
}

/// A document as stored: its snapshot and the updates saved after it, oldest first.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Saved {
    pub(crate) snapshot: Vec<u8>,
    pub(crate) updates: Vec<Vec<u8>>,
}

macro_rules! document {
    ($ty:ty) => {
        impl Document for $ty {
            fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError> {
                <$ty>::from_saved(snapshot, updates)
            }
            fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
                <$ty>::to_bytes(self)
            }
            fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError> {
                <$ty>::unsaved_changes(self)
            }
            fn mark_saved(&mut self) {
                <$ty>::mark_saved(self);
            }
        }
    };
}

document!(Workspace);
document!(PageDoc);

impl Store {
    /// Writes `doc`'s unsaved changes under `key`, at `now` (Unix milliseconds, from the caller so
    /// autosave can be tested with a fake clock). Returns whether anything was written.
    ///
    /// A document never saved before gets a snapshot; after that each save appends one update. When
    /// nothing changed, nothing is written. The document is marked saved only once the write
    /// succeeded, so a failed save is retried in full next time.
    pub fn save<D: Document>(&mut self, key: DocKey, doc: &mut D, now: i64) -> Result<bool, StoreError> {
        let id = key.to_string();
        if !self.has_doc(&id)? {
            self.conn.execute(
                "INSERT INTO docs (doc_id, snapshot, updated_at) VALUES (?1, ?2, ?3)",
                params![id, doc.to_bytes()?, now],
            )?;
        } else if let Some(update) = doc.unsaved_changes()? {
            self.conn.execute(
                "INSERT INTO doc_updates (doc_id, data, created_at) VALUES (?1, ?2, ?3)",
                params![id, update, now],
            )?;
        } else {
            return Ok(false);
        }
        doc.mark_saved();
        Ok(true)
    }

    /// Loads the document saved under `key`, or `None` if it was never saved.
    pub fn load<D: Document>(&self, key: DocKey) -> Result<Option<D>, StoreError> {
        let Some(saved) = self.read(&key.to_string())? else {
            return Ok(None);
        };
        Ok(Some(D::from_saved(&saved.snapshot, &saved.updates)?))
    }

    fn has_doc(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM docs WHERE doc_id = ?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    /// The snapshot and every update for `id`, oldest update first.
    pub(crate) fn read(&self, id: &str) -> Result<Option<Saved>, StoreError> {
        let snapshot: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT snapshot FROM docs WHERE doc_id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?;
        let Some(snapshot) = snapshot else {
            return Ok(None);
        };
        let mut stmt = self
            .conn
            .prepare_cached("SELECT data FROM doc_updates WHERE doc_id = ?1 ORDER BY seq")?;
        let updates = stmt
            .query_map([id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(Some(Saved { snapshot, updates }))
    }
}

#[cfg(test)]
mod tests;
