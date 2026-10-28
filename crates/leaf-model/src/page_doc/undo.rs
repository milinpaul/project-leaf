//! Undo and redo for a page, with the text cursor carried along.
//!
//! Each page command is one Loro commit, and Loro's `UndoManager` turns commits into undo steps
//! (commits closer together than the merge interval share one, so typing undoes as a burst). Each
//! step also remembers where the editor's caret was, so undo puts the cursor back where it was before
//! the change and redo where it was before the undo.

use std::sync::{Arc, Mutex};

use leaf_core::BlockId;
use loro::{LoroDoc, LoroValue, UndoItemMeta, UndoManager};

use super::{BlockIndex, PageDoc};
use crate::{DEFAULT_UNDO_MERGE_MS, ModelError};

/// Where the text cursor is: a block and a Unicode scalar position in its text.
///
/// Stored with each undo step by `BlockId`, not Loro's cursors, because undo can re-create a block
/// under a new `TreeID` (ADR 0006) while its `BlockId` never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    /// The block.
    pub block: BlockId,
    /// Position in its text.
    pub at: usize,
}

/// What an undo or redo did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum History {
    /// There was nothing to undo or redo.
    Nothing,
    /// A step was undone or redone; put the cursor here (if a caret was recorded with the step).
    Done {
        /// Where the caret belongs now.
        caret: Option<Caret>,
    },
}

/// The caret shared with Loro's undo callbacks: the one to record next, and the one a pop handed back.
#[derive(Default)]
pub(super) struct CaretSlots {
    current: Mutex<Option<Caret>>,
    restored: Mutex<Option<Caret>>,
}

/// The undo manager every page starts with, wired to record and return carets.
pub(super) fn manager(doc: &LoroDoc, slots: &Arc<CaretSlots>) -> UndoManager {
    let mut undo = UndoManager::new(doc);
    undo.set_merge_interval(DEFAULT_UNDO_MERGE_MS);
    let record = Arc::clone(slots);
    undo.set_on_push(Some(Box::new(move |_, _, _| UndoItemMeta {
        value: record
            .current
            .lock()
            .ok()
            .and_then(|c| *c)
            .map_or(LoroValue::Null, caret_to_value),
        cursors: Vec::new(),
    })));
    let restore = Arc::clone(slots);
    undo.set_on_pop(Some(Box::new(move |_, _, meta| {
        if let Ok(mut restored) = restore.restored.lock() {
            *restored = caret_from_value(&meta.value);
        }
    })));
    undo
}

impl PageDoc {
    /// Tells the page where the editor's caret is, so the next undo step (from a command, or from
    /// an undo creating a redo step) remembers it. Call before each command, undo and redo.
    pub fn set_caret(&self, caret: Option<Caret>) {
        if let Ok(mut current) = self.carets.current.lock() {
            *current = caret;
        }
    }

    /// Reverts the most recent undo step.
    pub fn undo(&mut self) -> Result<History, ModelError> {
        self.take_restored();
        let changed = self.undo.undo()?;
        self.after_history_change(changed)
    }

    /// Re-applies the most recently undone step.
    pub fn redo(&mut self) -> Result<History, ModelError> {
        self.take_restored();
        let changed = self.undo.redo()?;
        self.after_history_change(changed)
    }

    /// How close together, in milliseconds, commits must be to undo as one step. `0` makes every
    /// command its own step.
    pub fn set_undo_merge_interval(&mut self, ms: i64) {
        self.undo.set_merge_interval(ms);
    }

    fn take_restored(&self) -> Option<Caret> {
        self.carets.restored.lock().ok().and_then(|mut slot| slot.take())
    }

    /// Undo and redo can re-create block nodes under new `TreeID`s, so the index is rebuilt.
    fn after_history_change(&mut self, changed: bool) -> Result<History, ModelError> {
        if !changed {
            return Ok(History::Nothing);
        }
        self.index = BlockIndex::build(&self.tree)?;
        Ok(History::Done {
            caret: self.take_restored(),
        })
    }
}

fn caret_to_value(caret: Caret) -> LoroValue {
    let at = i64::try_from(caret.at).unwrap_or(i64::MAX);
    LoroValue::List(
        vec![
            LoroValue::String(caret.block.to_string().into()),
            LoroValue::I64(at),
        ]
        .into(),
    )
}

fn caret_from_value(value: &LoroValue) -> Option<Caret> {
    let LoroValue::List(items) = value else {
        return None;
    };
    match items.as_slice() {
        [LoroValue::String(block), LoroValue::I64(at)] => Some(Caret {
            block: block.parse().ok()?,
            at: usize::try_from(*at).ok()?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
