//! Undo and redo for the workspace doc.
//!
//! Each command is one Loro commit, and Loro's `UndoManager` turns commits into undo steps. Commits
//! closer together than the merge interval share a step, so typing a title undoes as one change.

use loro::{LoroDoc, UndoManager};

use super::{PageIndex, Workspace};
use crate::ModelError;

/// Commits closer together than this many milliseconds undo as one step (`BLOCK_MODEL.md`).
pub const DEFAULT_UNDO_MERGE_MS: i64 = 500;

/// The undo manager every workspace starts with.
pub(super) fn manager(doc: &LoroDoc) -> UndoManager {
    let mut undo = UndoManager::new(doc);
    undo.set_merge_interval(DEFAULT_UNDO_MERGE_MS);
    undo
}

impl Workspace {
    /// Reverts the most recent undo step. Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        let changed = self.undo.undo()?;
        self.after_history_change(changed)
    }

    /// Re-applies the most recently undone step. Returns `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        let changed = self.undo.redo()?;
        self.after_history_change(changed)
    }

    /// How close together, in milliseconds, commits must be to undo as one step. `0` makes every
    /// command its own step, which tests use to check each command on its own.
    pub fn set_undo_merge_interval(&mut self, ms: i64) {
        self.undo.set_merge_interval(ms);
    }

    /// Undo and redo can re-create page nodes under new `TreeID`s (ADR 0006), so the index is
    /// rebuilt from each page's permanent `id`.
    fn after_history_change(&mut self, changed: bool) -> Result<bool, ModelError> {
        if changed {
            self.index = PageIndex::build(&self.tree)?;
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests;
