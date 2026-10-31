//! Compaction (E03-S05, ADR-048): folding a document's saved updates back into its snapshot.
//!
//! Every save appends one small update, so a page edited for months would collect thousands of
//! them, and each load would replay them all. Once a document has more than [`COMPACT_AFTER`], the
//! save that crossed the line writes one fresh snapshot of exactly the same document and deletes the
//! updates it folded in, in one transaction: a crash or error part way leaves the old snapshot and
//! every update where they were.

use leaf_model::fold_saved;
use rusqlite::{TransactionBehavior, params};

use crate::docs::read;
use crate::{DocKey, Store, StoreError};

/// How many saved updates a document may collect before they are folded into its snapshot
/// (`BLOCK_MODEL.md`, "Persistence").
pub const COMPACT_AFTER: usize = 200;

impl Store {
    /// Folds `key`'s saved updates into a fresh snapshot if it has more than [`COMPACT_AFTER`].
    /// Returns whether it did. [`Store::save`] calls this after each update it appends.
    ///
    /// When an update can't be read, nothing is folded: the snapshot would lose it, and Leaf never
    /// deletes data it can't read (ADR-012). The document's "last saved" time stays the time of its
    /// last edit, because compaction changes nothing in it.
    pub fn compact(&mut self, key: DocKey) -> Result<bool, StoreError> {
        let id = key.to_string();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        // SQLite counts in i64; rusqlite reads no `usize` (0.41). A count never goes negative.
        let count: i64 = tx.query_row(
            "SELECT count(*) FROM doc_updates WHERE doc_id = ?1",
            [&id],
            |row| row.get(0),
        )?;
        if usize::try_from(count).map_or(true, |count| count <= COMPACT_AFTER) {
            return Ok(false);
        }
        let Some(saved) = read(&tx, &id)? else {
            return Ok(false);
        };
        let snapshot = match fold_saved(&saved.snapshot, &saved.updates) {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => {
                tracing::warn!(doc = %key, "not compacting: a saved update depends on a missing one");
                return Ok(false);
            }
            Err(err) => {
                tracing::warn!(doc = %key, %err, "not compacting: a saved update can't be read");
                return Ok(false);
            }
        };
        // The snapshot is written first, so an interruption between the two steps is the case the
        // transaction has to undo.
        tx.execute(
            "UPDATE docs SET snapshot = ?2, updated_at = max(updated_at,
                 (SELECT max(created_at) FROM doc_updates WHERE doc_id = ?1))
             WHERE doc_id = ?1",
            params![id, snapshot],
        )?;
        tx.execute("DELETE FROM doc_updates WHERE doc_id = ?1", [&id])?;
        tx.commit()?;
        Ok(true)
    }
}
