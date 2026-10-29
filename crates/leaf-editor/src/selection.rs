//! What is selected: text between two carets (in one block or across several), or whole blocks.
//!
//! A text selection is the caret plus a *tail*, the end that stays put while Shift+arrows move the
//! caret. Both are model anchors, so the selection follows edits made by anything else. Whole-block
//! selection is a separate mode: Cmd/Ctrl+A pressed twice, or a divider or image picked by Backspace or
//! Delete. In that mode Backspace and Delete remove the blocks, and Tab nests them (ADR 0021).

use std::ops::Range;

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, ModelError, PageCommand, PageDoc};

use crate::caret::{LineLayout, Motion, Vertical};
use crate::{EditorError, PageEditor};

/// A text selection inside one block. Selections across blocks are [`Selection::Text`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange {
    /// The block.
    pub block: BlockId,
    /// Unicode scalar positions, `start <= end`.
    pub range: Range<usize>,
}

/// What is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Text from `start` to `end`, in reading order; they may be in different blocks.
    Text {
        /// Where the selection begins.
        start: Caret,
        /// Where it ends.
        end: Caret,
    },
    /// Whole blocks, each with everything nested in it, in reading order.
    Blocks(Vec<BlockId>),
}

impl PageEditor {
    /// Selects `range` in `block`, with the caret at its end. Both ends snap to character starts.
    pub fn select(&mut self, block: BlockId, range: Range<usize>) -> Result<(), EditorError> {
        let start = self.set_caret(Caret {
            block,
            at: range.start,
        })?;
        self.set_caret(Caret { block, at: range.end })?;
        self.set_tail(start)
    }

    /// Selects from `tail` to `caret`, which may be in different blocks; the caret ends at `caret`.
    pub fn select_between(&mut self, tail: Caret, caret: Caret) -> Result<(), EditorError> {
        let tail = self.set_caret(tail)?;
        self.set_caret(caret)?;
        self.set_tail(tail)
    }

    /// Selects whole blocks (block selection mode). The caret stays where it was, for when the mode
    /// ends. Blocks not on the page are ignored; none at all ends the mode.
    pub fn select_blocks(&mut self, blocks: &[BlockId]) -> Result<(), EditorError> {
        let order = self.order()?;
        let picked: Vec<BlockId> = order.into_iter().filter(|b| blocks.contains(b)).collect();
        self.blocks = (!picked.is_empty()).then_some(picked);
        Ok(())
    }

    /// Cmd/Ctrl+A. The first press selects the caret's block text; the next selects every block. An
    /// empty block has nothing to select, so it goes straight to every block.
    pub fn select_all(&mut self) -> Result<Option<Selection>, EditorError> {
        if self.blocks.is_none()
            && let Some(caret) = self.caret()?
        {
            let len = self.doc.plain_text(caret.block)?.map_or(0, |t| t.chars().count());
            let whole = Selection::Text {
                start: Caret { at: 0, ..caret },
                end: Caret { at: len, ..caret },
            };
            if len > 0 && self.selection()? != Some(whole) {
                self.select(caret.block, 0..len)?;
                return self.selection();
            }
        }
        let all = self.doc.root_blocks();
        self.select_blocks(&all)?;
        self.selection()
    }

    /// Shift+Left/Right/Home/End/word moves: the caret moves and the other end stays, across blocks.
    pub fn extend(&mut self, motion: Motion) -> Result<Option<Selection>, EditorError> {
        let Some(tail) = self.tail_or_caret()? else {
            return Ok(None);
        };
        self.move_caret_from(motion)?;
        self.set_tail(tail)?;
        self.selection()
    }

    /// Shift+Up/Down: like [`PageEditor::move_vertically`], keeping the other end.
    pub fn extend_vertically(
        &mut self,
        direction: Vertical,
        layout: &impl LineLayout,
    ) -> Result<Option<Selection>, EditorError> {
        let Some(tail) = self.tail_or_caret()? else {
            return Ok(None);
        };
        self.blocks = None;
        self.move_vertically(direction, layout)?;
        self.set_tail(tail)?;
        self.selection()
    }

    /// What is selected now, after whatever changed since it was made. `None` when nothing is, or a
    /// text selection collapsed to the caret.
    pub fn selection(&mut self) -> Result<Option<Selection>, EditorError> {
        if let Some(blocks) = &self.blocks {
            let left: Vec<BlockId> = blocks.iter().copied().filter(|&b| self.doc.contains(b)).collect();
            return Ok((!left.is_empty()).then_some(Selection::Blocks(left)));
        }
        Ok(self
            .text_selection()?
            .map(|(start, end)| Selection::Text { start, end }))
    }

    /// The selected text as (start, end) in reading order, if any text is selected.
    pub(crate) fn text_selection(&mut self) -> Result<Option<(Caret, Caret)>, EditorError> {
        if self.blocks.is_some() {
            return Ok(None);
        }
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        let Some(tail) = self.caret.as_ref().and_then(|tracked| tracked.tail.clone()) else {
            return Ok(None);
        };
        let Some(other) = self.doc.resolve(&tail)? else {
            return Ok(None);
        };
        if other == caret {
            return Ok(None);
        }
        let order = self.order()?;
        let key = |c: &Caret| (order.iter().position(|&b| b == c.block), c.at);
        Ok(Some(if key(&other) < key(&caret) {
            (other, caret)
        } else {
            (caret, other)
        }))
    }

    /// The selected text when it lies in one block.
    pub(crate) fn text_range(&mut self) -> Result<Option<TextRange>, EditorError> {
        Ok(self.text_selection()?.and_then(|(start, end)| {
            (start.block == end.block).then_some(TextRange {
                block: start.block,
                range: start.at..end.at,
            })
        }))
    }

    /// The blocks a text selection touches, in reading order.
    pub(crate) fn blocks_between(&mut self, start: Caret, end: Caret) -> Result<Vec<BlockId>, EditorError> {
        let order = self.order()?;
        let at = |b| order.iter().position(|&x| x == b).unwrap_or(0);
        Ok(order[at(start.block)..=at(end.block)].to_vec())
    }

    fn tail_or_caret(&mut self) -> Result<Option<Caret>, EditorError> {
        if let Some((start, end)) = self.text_selection()? {
            let caret = self.caret()?;
            return Ok(Some(if caret == Some(start) { end } else { start }));
        }
        self.blocks = None;
        self.caret()
    }

    pub(crate) fn set_tail(&mut self, tail: Caret) -> Result<(), EditorError> {
        let anchor = self.doc.anchor(tail)?;
        if let Some(tracked) = &mut self.caret {
            tracked.tail = Some(anchor);
        }
        Ok(())
    }
}

/// Deletes the text from `start` to `end` (reading order, possibly across blocks), on the document
/// alone so it can run inside [`PageDoc::as_one_step`]. `order` is every block in reading order.
///
/// Across blocks, like any text editor: the end of the first block and the start of the last go, the
/// blocks in between go with everything nested in them, and what is left of the last block joins the
/// first. Blocks that hold the last one (its parents) can't simply be deleted, since that would delete
/// it too: their text is emptied and they are merged away, so their remaining children move up.
/// An image caption never takes in other text, so a selection ending or starting in one isn't joined.
pub(crate) fn delete_span(
    doc: &mut PageDoc,
    order: &[BlockId],
    start: Caret,
    end: Caret,
) -> Result<Caret, EditorError> {
    let clear = |doc: &mut PageDoc, id: BlockId, range: Range<usize>| -> Result<(), EditorError> {
        if !range.is_empty() {
            doc.apply(PageCommand::DeleteText { id, range })?;
        }
        Ok(())
    };
    let len = |doc: &PageDoc, id| -> Result<usize, EditorError> {
        Ok(doc.plain_text(id)?.map_or(0, |t| t.chars().count()))
    };
    if start.block == end.block {
        clear(doc, start.block, start.at..end.at)?;
        return Ok(start);
    }
    let mut holders = Vec::new();
    let mut parent = doc.parent(end.block)?;
    while let Some(p) = parent {
        holders.push(p);
        parent = doc.parent(p)?;
    }
    let position = |b: BlockId| order.iter().position(|&x| x == b).unwrap_or(0);
    let between = &order[position(start.block) + 1..position(end.block)];
    let (lifted, doomed): (Vec<BlockId>, Vec<BlockId>) =
        between.iter().copied().partition(|b| holders.contains(b));

    let start_len = len(doc, start.block)?;
    clear(doc, start.block, start.at..start_len)?;
    clear(doc, end.block, 0..end.at)?;
    for &holder in &lifted {
        let holder_len = len(doc, holder)?;
        clear(doc, holder, 0..holder_len)?;
    }
    if !doomed.is_empty() {
        doc.apply(PageCommand::DeleteBlocks { ids: doomed })?;
    }
    let is_image = |doc: &PageDoc, id| -> Result<bool, EditorError> {
        Ok(matches!(doc.kind(id)?, BlockKind::Image { .. }))
    };
    if is_image(doc, start.block)? || is_image(doc, end.block)? {
        return Ok(start);
    }
    for id in lifted.into_iter().chain([end.block]) {
        match doc.apply(PageCommand::MergeIntoPrevious { id }) {
            Ok(_) => {}
            Err(ModelError::MergeTargetNotText { .. }) => break,
            Err(err) => return Err(err.into()),
        }
    }
    Ok(start)
}

#[cfg(test)]
mod tests;
