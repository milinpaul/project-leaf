//! Saving and loading documents: a snapshot per document plus the updates saved after it.
//!
//! The first save of a document writes a full snapshot. Every later save appends one small Loro
//! update holding all commits since the previous save, so a save costs about as much as the edit,
//! not the page. Loading reads the snapshot, then the updates in the order they were saved.
//! Once a document has collected more than [`crate::COMPACT_AFTER`] updates, the save folds them back
//! into a fresh snapshot (compaction, E03-S05).

use std::fmt;

use leaf_core::{DatabaseId, PageId};
use leaf_model::{Database, ModelError, PageDoc, Workspace};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{Store, StoreError};

/// Which document a row belongs to: the workspace (page tree), one page's blocks, or one database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKey {
    /// The workspace document, stored as `doc_id = 'workspace'`.
    Workspace,
    /// A page's blocks, stored under the page's ID.
    Page(PageId),
    /// A database's columns, rows and cells (Phase 1, ADR-066), stored as `database:<id>` so it can never be
    /// mistaken for a page.
    Database(DatabaseId),
}

/// What a database's `doc_id` starts with.
pub(crate) const DATABASE_PREFIX: &str = "database:";

impl fmt::Display for DocKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace => f.write_str("workspace"),
            Self::Page(page) => page.fmt(f),
            Self::Database(database) => write!(f, "{DATABASE_PREFIX}{database}"),
        }
    }
}

/// A document the store can save: what saving needs, and nothing about how it was built.
///
/// Object-safe, so autosave can handle the workspace and its pages in one list. Implemented for
/// [`Workspace`] and [`PageDoc`], whose methods of the same names explain the details.
pub trait Savable {
    /// The whole document.
    fn to_bytes(&self) -> Result<Vec<u8>, ModelError>;
    /// Changes not yet on disk, as one update, or `None`.
    fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError>;
    /// Whether there are changes not yet on disk (cheap).
    fn has_unsaved_changes(&self) -> bool;
    /// Everything is on disk now.
    fn mark_saved(&self);
}

/// A [`Savable`] document the store can also load; the store only ever handles bytes.
pub trait Document: Savable + Sized {
    /// Rebuilds the document from its snapshot and later updates, in saved order.
    fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError>;
    /// Like `from_saved`, but skips damaged updates; returns their positions in `updates`.
    fn recover(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(Self, Vec<usize>), ModelError>;
}

/// A document as stored: its snapshot and the updates saved after it, oldest first.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Saved {
    pub(crate) snapshot: Vec<u8>,
    pub(crate) updates: Vec<Vec<u8>>,
    /// Each update's `seq`, to name a damaged one.
    pub(crate) seqs: Vec<i64>,
}

/// A document loaded despite damaged updates, and which `doc_updates` rows (by `seq`) were skipped.
///
/// The skipped rows stay on disk untouched: Leaf never deletes data it can't read.
#[derive(Debug)]
pub struct Recovered<D> {
    /// The document, with every readable update applied.
    pub doc: D,
    /// The `seq` of each update that couldn't be read; empty when nothing was damaged.
    pub skipped: Vec<i64>,
}

macro_rules! document {
    ($ty:ty) => {
        impl Savable for $ty {
            fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
                <$ty>::to_bytes(self)
            }
            fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError> {
                <$ty>::unsaved_changes(self)
            }
            fn has_unsaved_changes(&self) -> bool {
                <$ty>::has_unsaved_changes(self)
            }
            fn mark_saved(&self) {
                <$ty>::mark_saved(self);
            }
        }

        impl Document for $ty {
            fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError> {
                <$ty>::from_saved(snapshot, updates)
            }
            fn recover(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(Self, Vec<usize>), ModelError> {
                <$ty>::recover(snapshot, updates)
            }
        }
    };
}

document!(Workspace);
document!(PageDoc);
document!(Database);

impl Store {
    /// Writes `doc`'s unsaved changes under `key`, at `now` (Unix milliseconds, from the caller so
    /// autosave can be tested with a fake clock). Returns whether anything was written.
    ///
    /// A document never saved before gets a snapshot; after that each save appends one update. When
    /// nothing changed, nothing is written. The document is marked saved only once the write
    /// succeeded, so a failed save is retried in full next time.
    pub fn save<D: Savable + ?Sized>(&mut self, key: DocKey, doc: &D, now: i64) -> Result<bool, StoreError> {
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
        // The save itself succeeded; a compaction that fails leaves the saved updates in place, so
        // it is only logged, and tried again on the next save.
        if let Err(err) = self.compact(key) {
            tracing::warn!(doc = %key, %err, "could not compact the saved updates");
        }
        Ok(true)
    }

    /// Removes the document saved under `key`, snapshot and updates together, for a page deleted for
    /// good. Returns whether there was anything to remove.
    pub fn delete(&mut self, key: DocKey) -> Result<bool, StoreError> {
        let id = key.to_string();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM doc_updates WHERE doc_id = ?1", params![id])?;
        let removed = tx.execute("DELETE FROM docs WHERE doc_id = ?1", params![id])?;
        tx.commit()?;
        Ok(removed > 0)
    }

    /// Loads the document saved under `key`, or `None` if it was never saved.
    ///
    /// A damaged update (a crash mid-write, a bad disk sector) doesn't stop the document from
    /// opening: it is skipped with a warning in the log, losing only the edits it held. Use
    /// [`Store::recover`] to learn what was skipped.
    pub fn load<D: Document>(&self, key: DocKey) -> Result<Option<D>, StoreError> {
        Ok(self.recover(key)?.map(|recovered| {
            for seq in &recovered.skipped {
                tracing::warn!(doc = %key, seq, "skipped a damaged saved update");
            }
            recovered.doc
        }))
    }

    /// Loads the document saved under `key` like [`Store::load`], also reporting any damaged updates
    /// that were skipped. Fails if the snapshot itself is damaged.
    pub fn recover<D: Document>(&self, key: DocKey) -> Result<Option<Recovered<D>>, StoreError> {
        let Some(saved) = self.read(&key.to_string())? else {
            return Ok(None);
        };
        let failed = match D::from_saved(&saved.snapshot, &saved.updates) {
            Ok(doc) => {
                return Ok(Some(Recovered {
                    doc,
                    skipped: Vec::new(),
                }));
            }
            Err(err) => err,
        };
        let (doc, skipped) = D::recover(&saved.snapshot, &saved.updates)?;
        if skipped.is_empty() {
            return Err(failed.into()); // the updates were fine; the problem is elsewhere
        }
        let skipped = skipped
            .into_iter()
            .filter_map(|i| saved.seqs.get(i).copied())
            .collect();
        Ok(Some(Recovered { doc, skipped }))
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
        read(&self.conn, id)
    }
}

/// [`Store::read`] on any connection, so compaction can read inside its transaction.
pub(crate) fn read(conn: &Connection, id: &str) -> Result<Option<Saved>, StoreError> {
    let snapshot: Option<Vec<u8>> = conn
        .query_row("SELECT snapshot FROM docs WHERE doc_id = ?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let mut stmt = conn.prepare_cached("SELECT seq, data FROM doc_updates WHERE doc_id = ?1 ORDER BY seq")?;
    let rows: Vec<(i64, Vec<u8>)> = stmt
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let (seqs, updates) = rows.into_iter().unzip();
    Ok(Some(Saved {
        snapshot,
        updates,
        seqs,
    }))
}

#[cfg(test)]
mod tests;
