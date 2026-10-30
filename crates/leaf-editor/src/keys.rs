//! Enter, Backspace, Delete and Tab at the caret: the structural editing keys.
//!
//! Each key follows the canonical rules in `PRODUCT_SPEC.md` "Editor behaviour", and each press is
//! exactly one undo step, even when it needs several commands (Enter over a selection deletes it and
//! splits the block). Undo puts the caret back where it was before the key (ADR 0020).
//!
//! Blocks without text, dividers and images, can't hold the caret. Backspace right after one, or
//! Delete right before one, selects it instead ([`KeyOutcome::SelectBlock`]); pressing it again
//! deletes it. Over a text selection, even one across blocks, every key is still one undo step.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, ModelError, PageCommand, PageOutcome};

use crate::caret::{motion, nearest_above};
use crate::page_editor::{enter_in, insert_after};
use crate::selection::delete_span;
use crate::{EditorError, PageEditor, Selection, TextRange};

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
    /// Handles a structural key at the caret and selection. With whole blocks selected, Backspace and
    /// Delete remove them, Tab and Shift+Tab nest them, and other keys do nothing.
    /// With the slash menu open and something matching, Enter applies the highlighted entry instead.
    pub fn press(&mut self, key: Key) -> Result<KeyOutcome, EditorError> {
        if key == Key::Enter
            && self.slash_menu()?.is_some_and(|menu| !menu.items.is_empty())
            && let Some(caret) = self.slash_apply(None)?
        {
            return Ok(KeyOutcome::Caret(caret));
        }
        let outcome = self.handle(key)?;
        self.finish()?;
        Ok(outcome)
    }

    fn handle(&mut self, key: Key) -> Result<KeyOutcome, EditorError> {
        if let Some(Selection::Blocks(blocks)) = self.selection()? {
            return self.block_key(key, &blocks);
        }
        let Some(caret) = self.caret()? else {
            return Ok(KeyOutcome::Nothing);
        };
        let span = self.text_selection()?;
        // Undo puts the selection back as it is now.
        self.begin()?;
        // Inside a code block, Tab and Shift+Tab indent lines instead of nesting the block.
        if let Some(outcome) = self.code_tab(key, caret, span)? {
            return Ok(outcome);
        }
        // In a table cell, Tab moves between cells, Enter types a line break and Backspace and
        // Delete never merge cells (E16-S02). A selection across cells is handled below like any
        // other: deleting it empties the cells.
        let in_one_block = span.is_none_or(|(start, end)| start.block == end.block);
        if (in_one_block || matches!(key, Key::Tab | Key::ShiftTab))
            && let Some(outcome) = self.cell_key(key, caret)?
        {
            return match outcome {
                KeyOutcome::Caret(at) => self.set_caret(at).map(KeyOutcome::Caret),
                other => Ok(other),
            };
        }
        let outcome = match (key, span) {
            (Key::Enter, Some((start, end))) if start.block != end.block => self.enter_over(start, end)?,
            (Key::Enter | Key::ModEnter, _) => {
                let range = self.text_range()?;
                self.enter_key(caret, range, key == Key::ModEnter)?
            }
            (Key::Backspace | Key::Delete, Some((start, end))) => self.delete_selection(start, end)?,
            (Key::Backspace, None) => self.backspace_key(caret)?,
            (Key::Delete, None) => self.delete_key(caret)?,
            (Key::Tab | Key::ShiftTab, span) => {
                let ids = match span {
                    Some((start, end)) => self.blocks_between(start, end)?,
                    None => vec![caret.block],
                };
                self.nest(key, &ids)?;
                KeyOutcome::Caret(caret)
            }
        };
        match outcome {
            KeyOutcome::Caret(at) if span.is_some() && matches!(key, Key::Tab | Key::ShiftTab) => {
                // Nesting keeps the selection.
                let (start, end) = span.unwrap_or((at, at));
                let tail = if at == start { end } else { start };
                self.select_between(tail, at)?;
                Ok(KeyOutcome::Caret(at))
            }
            KeyOutcome::Caret(at) => self.set_caret(at).map(KeyOutcome::Caret),
            KeyOutcome::SelectBlock(block) => {
                self.select_blocks(&[block])?;
                Ok(outcome)
            }
            KeyOutcome::Nothing => Ok(outcome),
        }
    }

    /// Deletes whole blocks and everything nested in them, as one undo step. The caret goes to the
    /// end of the nearest text block above the first, else the start of the one below. A page is
    /// never left empty: an empty paragraph takes the last block's place.
    pub fn delete_blocks(&mut self, ids: &[BlockId]) -> Result<Option<Caret>, EditorError> {
        let Some(row) = self.view()?.iter().position(|row| ids.contains(&row.id)) else {
            return Ok(None);
        };
        self.begin()?;
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            doc.apply(PageCommand::DeleteBlocks { ids: ids.to_vec() })?;
            if doc.root_blocks().is_empty() {
                crate::page_editor::insert_block(doc, None, 0, BlockKind::Paragraph)?;
            }
            Ok(())
        })?;
        let caret = match nearest_above(self.view()?, row) {
            Some(caret) => Some(self.set_caret(caret)?),
            None => None,
        };
        self.finish()?;
        Ok(caret)
    }

    fn block_key(&mut self, key: Key, blocks: &[BlockId]) -> Result<KeyOutcome, EditorError> {
        match key {
            Key::Backspace | Key::Delete => Ok(match self.delete_blocks(blocks)? {
                Some(caret) => KeyOutcome::Caret(caret),
                None => KeyOutcome::Nothing,
            }),
            Key::Tab | Key::ShiftTab => {
                self.begin()?;
                self.nest(key, blocks)?;
                // Still selected, wherever they moved.
                self.select_blocks(blocks)?;
                Ok(KeyOutcome::Nothing)
            }
            Key::Enter | Key::ModEnter => Ok(KeyOutcome::Nothing),
        }
    }

    fn nest(&mut self, key: Key, ids: &[BlockId]) -> Result<(), EditorError> {
        // A selection reaching into a table nests the whole table.
        let ids = crate::table::whole_tables(&self.doc, ids)?;
        self.doc.apply(if key == Key::Tab {
            PageCommand::Indent { ids }
        } else {
            PageCommand::Outdent { ids }
        })?;
        Ok(())
    }

    /// Enter over a selection across blocks: the selection goes, then Enter happens where it started,
    /// all in one undo step.
    fn enter_over(&mut self, start: Caret, end: Caret) -> Result<KeyOutcome, EditorError> {
        let order = self.order()?;
        let caret = self.doc.as_one_step(|doc| -> Result<Caret, EditorError> {
            let at = delete_span(doc, &order, start, end)?;
            match doc.kind(at.block)? {
                // A cell takes a line break; it is never split.
                BlockKind::Code { .. } | BlockKind::TableCell => {
                    doc.apply(PageCommand::InsertText {
                        id: at.block,
                        pos: at.at,
                        text: "\n".into(),
                    })?;
                    Ok(Caret { at: at.at + 1, ..at })
                }
                BlockKind::Image { .. } => Ok(Caret {
                    block: insert_after(doc, at.block, BlockKind::Paragraph)?,
                    at: 0,
                }),
                _ => enter_in(doc, at.block, at.at),
            }
        })?;
        Ok(KeyOutcome::Caret(caret))
    }

    fn delete_selection(&mut self, start: Caret, end: Caret) -> Result<KeyOutcome, EditorError> {
        let order = self.order()?;
        let at = self.doc.as_one_step(|doc| delete_span(doc, &order, start, end))?;
        Ok(KeyOutcome::Caret(at))
    }

    /// Deletes one block with everything nested in it: see [`PageEditor::delete_blocks`].
    pub fn delete_block(&mut self, id: BlockId) -> Result<Option<Caret>, EditorError> {
        self.delete_blocks(&[id])
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

    fn backspace_key(&mut self, caret: Caret) -> Result<KeyOutcome, EditorError> {
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

    fn delete_key(&mut self, caret: Caret) -> Result<KeyOutcome, EditorError> {
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
