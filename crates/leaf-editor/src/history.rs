//! Undo and redo with the selection: what each editing action records, and how it comes back.
//!
//! Every editing action records the selection twice: just before it changes the page (so undo can
//! put back exactly what was selected, including text across blocks or whole selected blocks) and
//! right after (so redo puts back what the edit left, wherever the caret has gone since) (ADR 0023).

use leaf_model::{History, SavedSelection};

use crate::{EditorError, PageEditor, Selection};

impl PageEditor {
    /// Undoes the last change and restores the selection from just before it. `false` if there was
    /// nothing to undo.
    pub fn undo(&mut self) -> Result<bool, EditorError> {
        let history = self.doc.undo()?;
        Ok(self.restore(history))
    }

    /// Redoes the last undone change and restores the selection from just after it. `false` if
    /// there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EditorError> {
        let history = self.doc.redo()?;
        Ok(self.restore(history))
    }

    /// Records the selection as "before" for the change about to be made. Call before applying.
    pub(crate) fn begin(&mut self) -> Result<(), EditorError> {
        let selection = self.saved_selection()?;
        self.doc.set_selection(selection);
        Ok(())
    }

    /// Records the selection as "after" for the change just made. Call when the action is done.
    pub(crate) fn finish(&mut self) -> Result<(), EditorError> {
        let selection = self.saved_selection()?;
        self.doc.record_after(selection);
        Ok(())
    }

    fn saved_selection(&mut self) -> Result<Option<SavedSelection>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        let mut saved = SavedSelection::at(caret);
        match self.selection()? {
            Some(Selection::Text { start, end }) => {
                saved.tail = Some(if caret == start { end } else { start });
            }
            Some(Selection::Blocks(blocks)) => saved.blocks = blocks,
            // Cells are kept as the text selection between their corners, which gives them back.
            Some(Selection::Cells { .. }) => {
                if let Some((start, end)) = self.text_selection()? {
                    saved.tail = Some(if caret == start { end } else { start });
                }
            }
            None => {}
        }
        Ok(Some(saved))
    }

    /// Puts a recorded selection back. If it can't be (another device deleted its block meanwhile),
    /// the caret is left to follow the page as usual.
    fn restore(&mut self, history: History) -> bool {
        let History::Done { selection } = history else {
            return false;
        };
        self.composing = None;
        if let Some(saved) = selection {
            let restored = self.set_caret(saved.caret).and_then(|_| match saved.tail {
                Some(tail) => self.set_tail(tail),
                None => Ok(()),
            });
            if restored.is_ok() && !saved.blocks.is_empty() {
                // Selected blocks were all re-created by the undo, or still exist after the redo.
                let _ = self.select_blocks(&saved.blocks);
            }
        }
        true
    }
}

#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
mod tests;
