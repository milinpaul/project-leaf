//! Which changes of a document are already on disk.
//!
//! The store saves a document as a snapshot plus small updates. Each document remembers the Loro
//! version it was last saved at, so the next save writes only what changed since, as one update,
//! and writes nothing at all when nothing changed. Loro types stay inside the model; the store only
//! ever sees bytes.

use loro::{ExportMode, LoroDoc, VersionVector};

use crate::ModelError;

/// The version of a document that is safely saved.
pub(crate) struct SavedVersion(VersionVector);

impl SavedVersion {
    /// Everything in `doc` as it is now counts as saved.
    pub(crate) fn of(doc: &LoroDoc) -> Self {
        Self(doc.oplog_vv())
    }

    /// One Loro update with every change since the saved version, or `None` if there is none.
    pub(crate) fn changes(&self, doc: &LoroDoc) -> Result<Option<Vec<u8>>, ModelError> {
        if doc.oplog_vv() == self.0 {
            return Ok(None);
        }
        Ok(Some(doc.export(ExportMode::updates(&self.0))?))
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
