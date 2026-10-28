//! Editing one page's blocks: what to draw for each block, and what Enter, Backspace, Tab and the
//! toolbar do. Turns those actions into `PageCommand`s; never touches Loro itself.
//!
//! The interim block editor (E07-S10) draws one plain text input per block from [`BlockRow`]s. The
//! rules here carry over to the real editor (E04). The caret and its movement live in `caret`.

use leaf_core::BlockId;
pub use leaf_model::Caret;
use leaf_model::{BlockKind, BlockProp, ModelError, PageCommand, PageDoc, PageOutcome};

use crate::EditorError;
use crate::caret::Tracked;
use crate::view::{BlockRow, PageView};

/// What Backspace at the start of a block did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backspace {
    /// Something changed; put the caret here.
    Moved(Caret),
    /// The block above has no text (a divider or image): select it instead of merging.
    Select(BlockId),
    /// Nothing to do (the first block, or a block without text).
    Nothing,
}

/// A page's blocks being edited.
pub struct PageEditor {
    pub(crate) doc: PageDoc,
    /// The rows as last drawn, refreshed from the page's changes.
    view: PageView,
    /// Where the caret is, if it has been placed.
    pub(crate) caret: Option<Tracked>,
}

impl PageEditor {
    /// Starts editing `doc`.
    #[must_use]
    pub fn new(doc: PageDoc) -> Self {
        Self {
            doc,
            view: PageView::default(),
            caret: None,
        }
    }

    /// The page document, for read queries.
    #[must_use]
    pub fn doc(&self) -> &PageDoc {
        &self.doc
    }

    /// Every block in reading order, with its depth and style, built from scratch. For drawing use
    /// [`PageEditor::view`], which re-reads only what changed.
    pub fn rows(&self) -> Result<Vec<BlockRow>, EditorError> {
        Ok(PageView::build(&self.doc)?.rows().to_vec())
    }

    /// Every block in reading order, ready to draw, updated with only the blocks that changed since
    /// the last call.
    pub fn view(&mut self) -> Result<&[BlockRow], EditorError> {
        self.view.refresh(&self.doc)?;
        Ok(self.view.rows())
    }

    /// The first block, creating an empty paragraph on an empty page so there is somewhere to type.
    pub fn ensure_block(&mut self) -> Result<BlockId, EditorError> {
        if let Some(&first) = self.doc.root_blocks().first() {
            return Ok(first);
        }
        self.insert(None, 0, BlockKind::Paragraph)
    }

    /// Makes the block's text equal `text`, as the smallest delete and insert: only the changed middle
    /// is rewritten, so marks on the untouched start and end survive.
    pub fn set_text(&mut self, id: BlockId, text: &str) -> Result<(), EditorError> {
        let Some(old) = self.doc.plain_text(id)? else {
            return Ok(());
        };
        let (old, new): (Vec<char>, Vec<char>) = (old.chars().collect(), text.chars().collect());
        let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let max_suffix = old.len().min(new.len()) - prefix;
        let suffix = old
            .iter()
            .rev()
            .zip(new.iter().rev())
            .take(max_suffix)
            .take_while(|(a, b)| a == b)
            .count();
        let removed = prefix..old.len() - suffix;
        if !removed.is_empty() {
            self.doc.apply(PageCommand::DeleteText { id, range: removed })?;
        }
        let added: String = new[prefix..new.len() - suffix].iter().collect();
        if !added.is_empty() {
            self.doc.apply(PageCommand::InsertText {
                id,
                pos: prefix,
                text: added,
            })?;
        }
        Ok(())
    }

    /// Enter at the end of the block's text: see [`PageEditor::enter_at`]. Returns the block to focus.
    pub fn enter(&mut self, id: BlockId) -> Result<BlockId, EditorError> {
        let end = self.doc.plain_text(id)?.map_or(0, |text| text.chars().count());
        Ok(self.enter_at(id, end)?.block)
    }

    /// Enter with the cursor at `at` (a Unicode scalar position). Splits the block there: the text
    /// after the cursor moves, with its formatting, into a new block below, and the caret goes to its
    /// start. Lists and to-dos continue; anything else continues as a paragraph.
    ///
    /// On an **empty** list item or to-do, Enter leaves the list instead (`PRODUCT_SPEC.md` "Editor
    /// behaviour"): a nested item moves out one level, a top-level one becomes a paragraph, and no
    /// new block is added. That is how a run of list items is ended. Blocks without text (dividers)
    /// get an empty paragraph below.
    pub fn enter_at(&mut self, id: BlockId, at: usize) -> Result<Caret, EditorError> {
        let kind = self.doc.kind(id)?;
        let Some(text) = self.doc.plain_text(id)? else {
            let parent = self.doc.parent(id)?;
            let siblings = match parent {
                Some(parent) => self.doc.children(parent)?,
                None => self.doc.root_blocks(),
            };
            let index = siblings
                .iter()
                .position(|&b| b == id)
                .map_or(siblings.len(), |p| p + 1);
            let block = self.insert(parent, index, BlockKind::Paragraph)?;
            return Ok(Caret { block, at: 0 });
        };
        let is_list = matches!(
            kind,
            BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo { .. }
        );
        if is_list && text.is_empty() {
            if self.doc.parent(id)?.is_some() {
                self.outdent(id)?;
            } else {
                self.turn_into(id, BlockKind::Paragraph)?;
            }
            return Ok(Caret { block: id, at: 0 });
        }
        let PageOutcome::BlockCreated(block) = self.doc.apply(PageCommand::SplitBlock { id, pos: at })?
        else {
            unreachable!("SplitBlock always reports the block it created");
        };
        Ok(Caret { block, at: 0 })
    }

    /// Backspace with the cursor at the very start of the block (`PRODUCT_SPEC.md` "Editor
    /// behaviour"): a list, heading, quote or other non-paragraph turns into a paragraph, keeping its
    /// text; a paragraph merges into the block above, with the caret where the texts join. If the
    /// block above is a divider or image, it should be selected instead.
    pub fn backspace_at_start(&mut self, id: BlockId) -> Result<Backspace, EditorError> {
        match self.doc.kind(id)? {
            BlockKind::Paragraph => {}
            BlockKind::Divider | BlockKind::Image { .. } | BlockKind::Unknown { .. } => {
                return Ok(Backspace::Nothing);
            }
            _ => {
                self.turn_into(id, BlockKind::Paragraph)?;
                return Ok(Backspace::Moved(Caret { block: id, at: 0 }));
            }
        }
        match self.doc.apply(PageCommand::MergeIntoPrevious { id }) {
            Ok(PageOutcome::BlockMerged { into, at }) => Ok(Backspace::Moved(Caret { block: into, at })),
            Ok(_) => Ok(Backspace::Nothing),
            Err(ModelError::MergeTargetNotText { target }) => Ok(Backspace::Select(target)),
            Err(err) => Err(err.into()),
        }
    }

    /// Tab: nests the block under the one above, if it can.
    pub fn indent(&mut self, id: BlockId) -> Result<(), EditorError> {
        self.doc.apply(PageCommand::Indent { ids: vec![id] })?;
        Ok(())
    }

    /// Shift+Tab: moves the block out one level, if it can.
    pub fn outdent(&mut self, id: BlockId) -> Result<(), EditorError> {
        self.doc.apply(PageCommand::Outdent { ids: vec![id] })?;
        Ok(())
    }

    /// "Turn into": changes the block's kind, keeping its text.
    pub fn turn_into(&mut self, id: BlockId, kind: BlockKind) -> Result<(), EditorError> {
        self.doc.apply(PageCommand::SetBlockKind { id, kind })?;
        Ok(())
    }

    /// Ticks or unticks a to-do. Other blocks are left alone.
    pub fn toggle_checked(&mut self, id: BlockId) -> Result<(), EditorError> {
        if let BlockKind::Todo { checked } = self.doc.kind(id)? {
            self.doc.apply(PageCommand::SetProp {
                id,
                prop: BlockProp::Checked(!checked),
            })?;
        }
        Ok(())
    }

    /// The block just above `id` in reading order, for the up arrow.
    pub fn previous(&self, id: BlockId) -> Result<Option<BlockId>, EditorError> {
        let order = self.order()?;
        Ok(order
            .iter()
            .position(|&b| b == id)
            .and_then(|p| p.checked_sub(1))
            .map(|p| order[p]))
    }

    /// The block just below `id` in reading order, for the down arrow.
    pub fn next(&self, id: BlockId) -> Result<Option<BlockId>, EditorError> {
        let order = self.order()?;
        Ok(order
            .iter()
            .position(|&b| b == id)
            .and_then(|p| order.get(p + 1))
            .copied())
    }

    fn order(&self) -> Result<Vec<BlockId>, EditorError> {
        Ok(self.rows()?.into_iter().map(|row| row.id).collect())
    }

    fn insert(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        kind: BlockKind,
    ) -> Result<BlockId, EditorError> {
        let command = PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: String::new(),
        };
        let PageOutcome::BlockCreated(id) = self.doc.apply(command)? else {
            unreachable!("InsertBlock always reports the block it created");
        };
        Ok(id)
    }
}

#[cfg(test)]
mod tests;
