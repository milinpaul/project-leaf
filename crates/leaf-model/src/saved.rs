//! Which changes of a document are already on disk.
//!
//! The store saves a document as a snapshot plus small updates. Each document remembers the Loro
//! version it was last saved at, so the next save writes only what changed since, as one update,
//! and writes nothing at all when nothing changed. Loro types stay inside the model; the store only
//! ever sees bytes.

use std::sync::Mutex;

use loro::{ExportMode, LoroDoc, VersionVector};

use crate::ModelError;

/// The version of a document that is safely saved.
///
/// Behind a lock so saving needs only shared access: recording "this is on disk" changes no content,
/// and the app can save from a timer while only reading the document.
pub(crate) struct SavedVersion(Mutex<VersionVector>);

impl SavedVersion {
    /// Everything in `doc` as it is now counts as saved.
    pub(crate) fn of(doc: &LoroDoc) -> Self {
        Self(Mutex::new(doc.oplog_vv()))
    }

    /// Whether `doc` has changes since the saved version. Cheap: compares versions, exports nothing.
    pub(crate) fn is_behind(&self, doc: &LoroDoc) -> bool {
        self.0.lock().map_or(true, |saved| doc.oplog_vv() != *saved)
    }

    /// One Loro update with every change since the saved version, or `None` if there is none.
    pub(crate) fn changes(&self, doc: &LoroDoc) -> Result<Option<Vec<u8>>, ModelError> {
        let Ok(saved) = self.0.lock() else {
            // A panic while holding the lock: be safe and save everything.
            return Ok(Some(doc.export(ExportMode::Snapshot)?));
        };
        if doc.oplog_vv() == *saved {
            return Ok(None);
        }
        Ok(Some(doc.export(ExportMode::updates(&saved))?))
    }

    /// Everything in `doc` as it is now is on disk.
    pub(crate) fn update(&self, doc: &LoroDoc) {
        if let Ok(mut saved) = self.0.lock() {
            *saved = doc.oplog_vv();
        }
    }
}

/// Fills an empty `doc` from a saved snapshot and the updates saved after it.
pub(crate) fn import(doc: &LoroDoc, snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(), ModelError> {
    doc.import(snapshot)?;
    if !updates.is_empty() {
        doc.import_batch(updates)?;
    }
    Ok(())
}

/// Like [`import`], but imports each update on its own and skips the ones Loro rejects (damaged
/// bytes), returning their positions in `updates`. Used only after a normal load failed.
pub(crate) fn import_each(
    doc: &LoroDoc,
    snapshot: &[u8],
    updates: &[Vec<u8>],
) -> Result<Vec<usize>, ModelError> {
    doc.import(snapshot)?;
    Ok(updates
        .iter()
        .enumerate()
        .filter(|(_, update)| doc.import(update).is_err())
        .map(|(i, _)| i)
        .collect())
}

/// Folds a saved snapshot and the updates saved after it into one new snapshot of exactly the same
/// document, history included, so it keeps merging with copies elsewhere (compaction, E03-S05).
///
/// Works on bytes alone, for pages and the workspace alike, and never looks inside: data from a
/// newer Leaf is carried over untouched. Returns `None` when an update is waiting for a change that
/// isn't saved (a lost row): folding then would drop it, so the saved form must stay as it is. A
/// damaged update is an error, for the same reason.
pub fn fold_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Option<Vec<u8>>, ModelError> {
    let doc = LoroDoc::new();
    if doc.import(snapshot)?.pending.is_some() {
        return Ok(None);
    }
    if !updates.is_empty() && doc.import_batch(updates)?.pending.is_some() {
        return Ok(None);
    }
    Ok(Some(doc.export(ExportMode::Snapshot)?))
}

#[cfg(test)]
mod tests;
