//! Reading bytes into a Loro document without letting damaged bytes crash Leaf (E08-S11, ADR-088).
//!
//! Loro returns errors for most bad input, but fuzzing found bytes that trip an assertion inside its decoder
//! instead (a panic). Saved bytes come from disk, where a bad sector, a half-copied folder or a crafted
//! workspace can put anything, so every import in the model goes through here and a panic inside Loro
//! becomes [`ModelError::Damaged`]. The panic found so far happens while Loro reads an update's header,
//! before it changes the document; loads import into a fresh document and drop it on error either way.

use std::panic::{AssertUnwindSafe, catch_unwind};

use loro::{ImportStatus, LoroDoc, LoroError};

use crate::ModelError;

/// [`LoroDoc::import`], with a panic inside Loro turned into an error.
pub(crate) fn import(doc: &LoroDoc, bytes: &[u8]) -> Result<ImportStatus, ModelError> {
    guarded(|| doc.import(bytes))
}

/// [`LoroDoc::import_batch`], with a panic inside Loro turned into an error.
pub(crate) fn import_batch(doc: &LoroDoc, updates: &[Vec<u8>]) -> Result<ImportStatus, ModelError> {
    guarded(|| doc.import_batch(updates))
}

fn guarded(read: impl FnOnce() -> Result<ImportStatus, LoroError>) -> Result<ImportStatus, ModelError> {
    // `AssertUnwindSafe`: after a panic the caller gets an error and, on every load path, drops the
    // document, so no half-read state is used.
    match catch_unwind(AssertUnwindSafe(read)) {
        Ok(status) => Ok(status?),
        Err(panic) => {
            let message = panic
                .downcast_ref::<&str>()
                .map(ToString::to_string)
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            Err(ModelError::Damaged(message))
        }
    }
}
