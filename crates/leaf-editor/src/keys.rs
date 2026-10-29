//! Enter, Backspace, Delete and Tab at the caret: the structural editing keys.
//!
//! Each key follows the canonical rules in `PRODUCT_SPEC.md` "Editor behaviour", and each press is
//! exactly one undo step, even when it needs several commands (Enter over a selection deletes it and
//! splits the block). Undo puts the caret back where it was before the key (ADR 0020).
//!
//! Blocks without text, dividers and images, can't hold the caret. Backspace right after one, or
//! Delete right before one, asks for it to be selected instead ([`KeyOutcome::SelectBlock`]). A
//! selected block is deleted with [`PageEditor::delete_block`]; drawing the selection is E04-S06.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, ModelError, PageCommand, PageOutcome};

use crate::caret::{motion, nearest_above};
use crate::page_editor::{enter_in, insert_after};
use crate::{EditorError, PageEditor, TextRange};

/// A structural editing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Enter: split the block, or a newline in code.
    Enter,
    /// Cmd+Enter (Ctrl+Enter elsewhere): leave a code block.
    ModEnter,
    /// Backspace: delete backwards; at a block's start, convert or merge it.
    Backspace,
    /// Delete (Fn+Backspace on Mac laptops): delete forwards; at a block's end, merge the next one in.
    Delete,
    /// Tab: nest the block under the one above.
    Tab,
    /// Shift+Tab: move the block out one level.
    ShiftTab,
}

/// What a key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The caret is here now; the page may have changed.
    Caret(Caret),
    /// The neighbouring divider or image should be selected instead; nothing changed.
    SelectBlock(BlockId),
    /// No caret yet, so nothing happened.
    Nothing,
}

impl PageEditor {
    /// Handles a structural key at the caret (and its selection, if any).
    pub fn press(&mut self, key: Key) -> Result<KeyOutcome, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(KeyOutcome::Nothing);
        };
        let selection = self.selection()?;
        // Undo puts the caret back here.
        self.doc.set_caret(Some(caret));
        let outcome = match key {
            Key::Enter => self.enter_key(caret, selection, false)?,
            Key::ModEnter => self.enter_key(caret, selection, true)?,
            Key::Backspace => self.backspace_key(caret, selection)?,
            Key::Delete => self.delete_key(caret, selection)?,
            Key::Tab | Key::ShiftTab => {
                let ids = vec![caret.block];
                self.doc.apply(if key == Key::Tab {
                    PageCommand::Indent { ids }
                } else {
                    PageCommand::Outdent { ids }
                })?;
                KeyOutcome::Caret(caret)
            }
        };
        if let KeyOutcome::Caret(at) = outcome {
            return self.set_caret(at).map(KeyOutcome::Caret);
        }
        Ok(outcome)
    }

    /// Deletes a block and everything nested in it, as one undo step: a selected divider or image, or
    /// a block from its menu. The caret goes to the end of the nearest text block above, else the
    /// start of the one below. A page is never left empty: an empty paragraph takes the last block's
    /// place.
    pub fn delete_block(&mut self, id: BlockId) -> Result<Option<Caret>, EditorError> {
        let Some(row) = self.view()?.iter().position(|row| row.id == id) else {
            return Ok(None);
        };
        let before = self.caret()?;
        self.doc.set_caret(before);
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            doc.apply(PageCommand::DeleteBlocks { ids: vec![id] })?;
            if doc.root_blocks().is_empty() {
                crate::page_editor::insert_block(doc, None, 0, BlockKind::Paragraph)?;
            }
            Ok(())
        })?;
        match nearest_above(self.view()?, row) {
            Some(caret) => self.set_caret(caret).map(Some),
            None => Ok(None),
        }
    }

    fn enter_key(
        &mut self,
        caret: Caret,
        selection: Option<TextRange>,
        modified: bool,
    ) -> Result<KeyOutcome, EditorError> {
        let block = caret.block;
        let kind = self.doc.kind(block)?;
        let text = self.doc.plain_text(block)?.unwrap_or_default();
        let len = text.chars().count();
        let at = selection.as_ref().map_or(caret.at, |s| s.range.start);
        let outcome = match kind {
            BlockKind::Code { .. } if modified => self.leave_code(block, 0)?,
            BlockKind::Code { .. } if selection.is_none() && caret.at == len && text.ends_with("\n\n") => {
                self.leave_code(block, 2)?
            }
            BlockKind::Code { .. } => {
                let range = selection.map_or(caret.at..caret.at, |s| s.range);
                self.doc.apply(PageCommand::ReplaceText {
                    id: block,
                    range,
                    text: "\n".into(),
                })?;
                Caret { block, at: at + 1 }
            }
            _ if modified => caret,
            // A caption isn't split: Enter adds a paragraph below the image.
            BlockKind::Image { .. } => Caret {
                block: insert_after(&mut self.doc, block, BlockKind::Paragraph)?,
                at: 0,
            },
            _ => self.doc.as_one_step(|doc| {
                if let Some(selection) = &selection {
                    doc.apply(PageCommand::DeleteText {
                        id: block,
                        range: selection.range.clone(),
                    })?;
                }
                enter_in(doc, block, at)
            })?,
        };
        Ok(KeyOutcome::Caret(outcome))
    }

    /// Leaves a code block into a new paragraph below it, after removing `trim` trailing newlines.
    fn leave_code(&mut self, block: BlockId, trim: usize) -> Result<Caret, EditorError> {
        let len = self.doc.plain_text(block)?.map_or(0, |t| t.chars().count());
        self.doc.as_one_step(|doc| {
            if trim > 0 {
                doc.apply(PageCommand::DeleteText {
                    id: block,
                    range: len - trim..len,
                })?;
            }
            Ok(Caret {
                block: insert_after(doc, block, BlockKind::Paragraph)?,
                at: 0,
            })
        })
    }

    fn backspace_key(
        &mut self,
        caret: Caret,
        selection: Option<TextRange>,
    ) -> Result<KeyOutcome, EditorError> {
        if let Some(selection) = selection {
            return self.delete_range(selection);
        }
        let block = caret.block;
        if caret.at > 0 {
            let text = self.doc.plain_text(block)?.unwrap_or_default();
            let start = motion::previous_grapheme(&text, caret.at);
            return self.delete_range(TextRange {
                block,
                range: start..caret.at,
            });
        }
        if matches!(self.doc.kind(block)?, BlockKind::Image { .. }) {
            return Ok(KeyOutcome::SelectBlock(block));
        }
        Ok(match self.backspace_at_start(block)? {
            crate::Backspace::Moved(to) => KeyOutcome::Caret(to),
            crate::Backspace::Select(target) => KeyOutcome::SelectBlock(target),
            crate::Backspace::Nothing => KeyOutcome::Caret(caret),
        })
    }

    fn delete_key(&mut self, caret: Caret, selection: Option<TextRange>) -> Result<KeyOutcome, EditorError> {
        if let Some(selection) = selection {
            return self.delete_range(selection);
        }
        let block = caret.block;
        let text = self.doc.plain_text(block)?.unwrap_or_default();
        if caret.at < text.chars().count() {
            let end = motion::next_grapheme(&text, caret.at);
            return self.delete_range(TextRange {
                block,
                range: caret.at..end,
            });
        }
        // At the end: the next block's text joins this one. Captions take nothing in.
        if matches!(self.doc.kind(block)?, BlockKind::Image { .. }) {
            return Ok(KeyOutcome::Caret(caret));
        }
        let Some(next) = self.next(block)? else {
            return Ok(KeyOutcome::Caret(caret));
        };
        let next_kind = self.doc.kind(next)?;
        if !next_kind.has_text() || matches!(next_kind, BlockKind::Image { .. }) {
            return Ok(KeyOutcome::SelectBlock(next));
        }
        match self.doc.apply(PageCommand::MergeIntoPrevious { id: next }) {
            Ok(PageOutcome::BlockMerged { into, at }) => Ok(KeyOutcome::Caret(Caret { block: into, at })),
            Ok(_) | Err(ModelError::MergeTargetNotText { .. }) => Ok(KeyOutcome::Caret(caret)),
            Err(err) => Err(err.into()),
        }
    }

    fn delete_range(&mut self, selection: TextRange) -> Result<KeyOutcome, EditorError> {
        let at = selection.range.start;
        self.doc.apply(PageCommand::DeleteText {
            id: selection.block,
            range: selection.range,
        })?;
        Ok(KeyOutcome::Caret(Caret {
            block: selection.block,
            at,
        }))
    }
}

#[cfg(test)]
mod tests;
