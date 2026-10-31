//! Undo and redo for a page, with the editor's selection carried along.
//!
//! Each page command is one Loro commit, and Loro's `UndoManager` turns commits into undo steps
//! (commits closer together than the merge interval share one, so typing undoes as a burst). Each
//! step also remembers the editor's selection twice: as it was *before* the change, which undo puts
//! back, and as it was right *after*, which redo puts back (ADR-023).
//!
//! Loro carries one value per step and hands it to `on_pop`/`on_push`. We store a step number there
//! and keep the two selections in a table here: a step's number survives undo and redo (the pop of
//! one stack is followed by the push onto the other, which reuses the popped number).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leaf_core::BlockId;
use loro::{Frontiers, LoroDoc, LoroValue, UndoItemMeta, UndoManager, UndoOrRedo};

use super::{BlockIndex, PageDoc};
use crate::{DEFAULT_UNDO_MERGE_MS, ModelError};

/// Where the text cursor is: a block and a Unicode scalar position in its text.
///
/// Stored with each undo step by `BlockId`, not Loro's cursors, because undo can re-create a block
/// under a new `TreeID` (ADR-006) while its `BlockId` never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    /// The block.
    pub block: BlockId,
    /// Position in its text.
    pub at: usize,
}

/// The editor's selection as an undo step stores it: the caret, the other end of a text selection,
/// and any whole blocks selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSelection {
    /// Where the caret was.
    pub caret: Caret,
    /// The other end of the selected text, if text was selected.
    pub tail: Option<Caret>,
    /// Whole blocks selected, in reading order; empty unless in block selection mode.
    pub blocks: Vec<BlockId>,
}

impl SavedSelection {
    /// Just a caret, nothing selected.
    #[must_use]
    pub fn at(caret: Caret) -> Self {
        Self {
            caret,
            tail: None,
            blocks: Vec::new(),
        }
    }
}

/// What an undo or redo did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum History {
    /// There was nothing to undo or redo.
    Nothing,
    /// A step was undone or redone; restore this selection (if one was recorded with the step).
    Done {
        /// For undo, the selection before the change; for redo, the one right after it.
        selection: Option<SavedSelection>,
    },
}

impl History {
    /// The caret to restore, if any.
    #[must_use]
    pub fn caret(&self) -> Option<Caret> {
        match self {
            Self::Done {
                selection: Some(selection),
            } => Some(selection.caret),
            _ => None,
        }
    }
}

/// A step's selections.
#[derive(Default)]
struct Step {
    before: Option<SavedSelection>,
    after: Option<SavedSelection>,
}

/// State shared with Loro's undo callbacks.
#[derive(Default)]
pub(super) struct CaretSlots {
    /// The selection to record as "before" for the next new step.
    current: Mutex<Option<SavedSelection>>,
    /// The step just popped by undo or redo, so the push that follows reuses its number.
    popped: Mutex<Option<i64>>,
    /// Steps by number.
    steps: Mutex<HashMap<i64, Step>>,
    next: Mutex<i64>,
    /// The document's version when the selection was last set, so `record_after` can tell whether
    /// anything changed since.
    set_at: Mutex<Option<Frontiers>>,
}

impl CaretSlots {
    /// A number for a new step, remembering `current` as its "before".
    fn new_step(&self) -> i64 {
        let id = self.next.lock().map_or(0, |mut next| {
            *next += 1;
            *next
        });
        let before = self.current.lock().ok().and_then(|c| c.clone());
        if let Ok(mut steps) = self.steps.lock() {
            steps.insert(id, Step { before, after: None });
        }
        id
    }
}

/// The merge interval as configured, and whether a separate step is waiting for the next commit to
/// end it (see [`PageDoc::as_separate_step`]).
pub(super) struct Merging {
    ms: i64,
    restore_after_next_commit: bool,
}

impl Default for Merging {
    fn default() -> Self {
        Self {
            ms: DEFAULT_UNDO_MERGE_MS,
            restore_after_next_commit: false,
        }
    }
}

/// The undo manager every page starts with, wired to number steps.
pub(super) fn manager(doc: &LoroDoc, slots: &Arc<CaretSlots>) -> UndoManager {
    let mut undo = UndoManager::new(doc);
    undo.set_merge_interval(DEFAULT_UNDO_MERGE_MS);
    // Repairs after a merge put the page's rules back; they aren't the user's edits to undo.
    undo.add_exclude_origin_prefix(super::repair::REPAIR_ORIGIN);
    let record = Arc::clone(slots);
    undo.set_on_push(Some(Box::new(move |_, _, _| {
        // After a pop, this push is the same step moving to the other stack.
        let popped = record.popped.lock().ok().and_then(|mut p| p.take());
        let id = popped.unwrap_or_else(|| record.new_step());
        UndoItemMeta {
            value: LoroValue::I64(id),
            cursors: Vec::new(),
        }
    })));
    let restore = Arc::clone(slots);
    undo.set_on_pop(Some(Box::new(move |_, _, meta| {
        if let (LoroValue::I64(id), Ok(mut popped)) = (meta.value, restore.popped.lock()) {
            *popped = Some(id);
        }
    })));
    undo
}

impl PageDoc {
    /// Tells the page the editor's selection, so the next new undo step remembers it as "before".
    /// Call before each command.
    pub fn set_selection(&self, selection: Option<SavedSelection>) {
        if let Ok(mut current) = self.carets.current.lock() {
            *current = selection;
        }
        if let Ok(mut set_at) = self.carets.set_at.lock() {
            *set_at = Some(self.doc.oplog_frontiers());
        }
    }

    /// [`PageDoc::set_selection`] with just a caret.
    pub fn set_caret(&self, caret: Option<Caret>) {
        self.set_selection(caret.map(SavedSelection::at));
    }

    /// Records the selection right after the latest change, for redo to put back. Call after each
    /// editing action, paired with the [`PageDoc::set_selection`] made before it. Steps merged by the
    /// merge interval keep the last one recorded. If nothing changed since that call (or there was
    /// none), no step is touched, so an earlier step's is never overwritten.
    pub fn record_after(&self, selection: Option<SavedSelection>) {
        let set_at = self
            .carets
            .set_at
            .lock()
            .ok()
            .and_then(|mut set_at| set_at.take());
        if set_at.is_none_or(|set_at| set_at == self.doc.oplog_frontiers()) {
            return;
        }
        let Some(LoroValue::I64(id)) = self.undo.top_undo_value() else {
            return;
        };
        if let Ok(mut steps) = self.carets.steps.lock()
            && let Some(step) = steps.get_mut(&id)
        {
            step.after = selection;
        }
    }

    /// Reverts the most recent undo step.
    pub fn undo(&mut self) -> Result<History, ModelError> {
        self.history(UndoOrRedo::Undo)
    }

    /// Re-applies the most recently undone step.
    pub fn redo(&mut self) -> Result<History, ModelError> {
        self.history(UndoOrRedo::Redo)
    }

    fn history(&mut self, kind: UndoOrRedo) -> Result<History, ModelError> {
        let id = match kind {
            UndoOrRedo::Undo => self.undo.top_undo_value(),
            UndoOrRedo::Redo => self.undo.top_redo_value(),
        };
        let changed = match kind {
            UndoOrRedo::Undo => self.undo.undo()?,
            UndoOrRedo::Redo => self.undo.redo()?,
        };
        // A pop with no push after it (nothing changed) must not leak its number to the next step.
        if let Ok(mut popped) = self.carets.popped.lock() {
            *popped = None;
        }
        if !changed {
            return Ok(History::Nothing);
        }
        // Undo and redo can re-create block nodes under new `TreeID`s, so the index is rebuilt.
        self.index = BlockIndex::build(&self.tree)?;
        let selection = match (id, self.carets.steps.lock()) {
            (Some(LoroValue::I64(id)), Ok(steps)) => steps.get(&id).and_then(|step| match kind {
                UndoOrRedo::Undo => step.before.clone(),
                // Without a recorded "after" (an edit made outside the editor), the "before" is the
                // best guess.
                UndoOrRedo::Redo => step.after.clone().or_else(|| step.before.clone()),
            }),
            _ => None,
        };
        Ok(History::Done { selection })
    }

    /// How close together, in milliseconds, commits must be to undo as one step. `0` makes every
    /// command its own step.
    pub fn set_undo_merge_interval(&mut self, ms: i64) {
        self.merge = Merging {
            ms,
            restore_after_next_commit: false,
        };
        self.undo.set_merge_interval(ms);
    }

    /// Like [`PageDoc::as_one_step`], and also never merged with the step before or after it by the
    /// merge interval. For an automatic change right after typing, such as a Markdown shortcut turning
    /// "# " into a heading: undo must take back the conversion alone, giving back the typed "# ".
    pub fn as_separate_step<T, E: From<ModelError>>(
        &mut self,
        steps: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        // With no merge interval, the step's first commit starts a new step; the rest join it as a
        // group. The interval stays off until the next commit, which then starts a step of its own.
        self.undo.set_merge_interval(0);
        let result = self.as_one_step(steps);
        self.merge.restore_after_next_commit = true;
        result
    }

    /// Called after every commit: ends a separate step by turning the merge interval back on.
    pub(super) fn after_commit(&mut self) {
        if self.merge.restore_after_next_commit && !self.undo.is_paused() {
            self.merge.restore_after_next_commit = false;
            self.undo.set_merge_interval(self.merge.ms);
        }
    }

    /// Runs `steps`, which apply several commands, as **one** undo step. For edits a user sees as one
    /// action but that take more than one command: Enter over a selection deletes it and splits the
    /// block; leaving a code block trims its empty lines and adds a paragraph. Each command is still
    /// its own commit, so saving and other devices see them as usual.
    ///
    /// If a step fails, the commands before it stay applied (commands check their input before
    /// writing, so none is half done), and undo still reverts them together.
    pub fn as_one_step<T, E: From<ModelError>>(
        &mut self,
        steps: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        self.undo.group_start().map_err(ModelError::from)?;
        let result = steps(self);
        self.undo.group_end();
        result
    }
}

#[cfg(test)]
mod tests;
