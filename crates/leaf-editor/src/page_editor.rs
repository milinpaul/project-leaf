//! Editing one page's blocks: what to draw for each block, and what Enter, Backspace, Tab and the
//! toolbar do. Turns those actions into `PageCommand`s; never touches Loro itself.
//!
//! The interim block editor (E07-S10) draws one plain text input per block from [`BlockRow`]s. The
//! rules here carry over to the real editor (E04). The caret and its movement live in `caret`.

use leaf_core::BlockId;
pub use leaf_model::Caret;
use leaf_model::{BlockKind, BlockProp, ModelError, PageCommand, PageDoc, PageOutcome};

use crate::EditorError;
use crate::block_menu::OpenMenu;
use crate::caret::Tracked;
use crate::format::Pending;
use crate::image::ImageRequest;
use crate::input::Composing;
use crate::link_menu::LinkMenuState;
use crate::slash::Slash;
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
    /// The input method's draft while a composition is going on.
    pub(crate) composing: Option<Composing>,
    /// Whole blocks selected (block selection mode), in reading order; `None` outside the mode.
    pub(crate) blocks: Option<Vec<BlockId>>,
    /// Mark toggles waiting for the next typed text (Cmd+B with no selection).
    pub(crate) pending: Option<Pending>,
    /// The slash menu, while it is open.
    pub(crate) slash: Option<Slash>,
    /// The page-link menu (`[[`), while it is open.
    pub(crate) link_menu: Option<LinkMenuState>,
    /// The block menu, while it is open.
    pub(crate) menu: Option<OpenMenu>,
    /// A picture asked for from the slash menu, until it arrives or the picker is cancelled.
    pub(crate) image: Option<ImageRequest>,
    /// The offer to make a pasted web address a web bookmark, while it stands (E15-S03).
    pub(crate) offer: Option<crate::web_bookmark::OfferState>,
    /// A web bookmark's address asked for from the slash menu, until it is given or cancelled.
    pub(crate) address: Option<crate::web_bookmark::AddressRequest>,
    /// Web bookmarks waiting for their preview to be fetched, and whether fetching is allowed (E15-S05).
    pub(crate) previews: crate::preview_requests::PreviewRequests,
}

impl PageEditor {
    /// Starts editing `doc`.
    #[must_use]
    pub fn new(doc: PageDoc) -> Self {
        Self {
            doc,
            view: PageView::default(),
            caret: None,
            composing: None,
            blocks: None,
            pending: None,
            slash: None,
            link_menu: None,
            menu: None,
            image: None,
            offer: None,
            address: None,
            previews: crate::preview_requests::PreviewRequests::default(),
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
    /// start. Lists and to-dos continue; anything else continues as a paragraph. At the very start
    /// of a block with text, an empty paragraph goes above instead and the block keeps its kind.
    ///
    /// On an **empty** list item or to-do, Enter leaves the list instead (`PRODUCT_SPEC.md` "Editor
    /// behaviour"): a nested item moves out one level, a top-level one becomes a paragraph, and no
    /// new block is added. That is how a run of list items is ended. Blocks without text (dividers)
    /// get an empty paragraph below.
    pub fn enter_at(&mut self, id: BlockId, at: usize) -> Result<Caret, EditorError> {
        enter_in(&mut self.doc, id, at)
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

    /// Ticks or unticks a to-do, or an action item off and on (E19-S02). Other blocks are left alone.
    pub fn toggle_checked(&mut self, id: BlockId) -> Result<(), EditorError> {
        let prop = match self.doc.kind(id)? {
            BlockKind::Todo { checked } => BlockProp::Checked(!checked),
            BlockKind::Task { done, .. } => BlockProp::TaskDone(!done),
            _ => return Ok(()),
        };
        self.doc.apply(PageCommand::SetProp { id, prop })?;
        Ok(())
    }

    /// Gives a callout another tone (E21-S02), one undo step. Other blocks are left alone, so a picker
    /// still open after its callout changed kind does nothing.
    pub fn set_callout_tone(
        &mut self,
        id: BlockId,
        tone: leaf_model::CalloutTone,
    ) -> Result<(), EditorError> {
        if !matches!(self.doc.kind(id)?, BlockKind::Callout { .. }) {
            return Ok(());
        }
        self.doc.apply(PageCommand::SetProp {
            id,
            prop: BlockProp::CalloutTone(tone),
        })?;
        Ok(())
    }

    /// The block just above `id` in reading order, for the up arrow.
    pub fn previous(&mut self, id: BlockId) -> Result<Option<BlockId>, EditorError> {
        let order = self.order()?;
        Ok(order
            .iter()
            .position(|&b| b == id)
            .and_then(|p| p.checked_sub(1))
            .map(|p| order[p]))
    }

    /// The block just below `id` in reading order, for the down arrow.
    pub fn next(&mut self, id: BlockId) -> Result<Option<BlockId>, EditorError> {
        let order = self.order()?;
        Ok(order
            .iter()
            .position(|&b| b == id)
            .and_then(|p| order.get(p + 1))
            .copied())
    }

    /// Every block in reading order, from the incrementally kept view: typing in one block costs no
    /// rebuild of the whole page.
    pub(crate) fn order(&mut self) -> Result<Vec<BlockId>, EditorError> {
        Ok(self.view()?.iter().map(|row| row.id).collect())
    }

    fn insert(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        kind: BlockKind,
    ) -> Result<BlockId, EditorError> {
        insert_block(&mut self.doc, parent, index, kind)
    }
}

/// Enter at `at` in block `id`, on the document alone so it can run inside
/// [`PageDoc::as_one_step`]. See [`PageEditor::enter_at`].
pub(crate) fn enter_in(doc: &mut PageDoc, id: BlockId, at: usize) -> Result<Caret, EditorError> {
    let kind = doc.kind(id)?;
    let Some(text) = doc.plain_text(id)? else {
        let block = insert_after(doc, id, BlockKind::Paragraph)?;
        return Ok(Caret { block, at: 0 });
    };
    let is_list = matches!(
        kind,
        BlockKind::BulletedList | BlockKind::NumberedList | BlockKind::Todo { .. } | BlockKind::Task { .. }
    );
    if is_list && text.is_empty() {
        if doc.parent(id)?.is_some() {
            doc.apply(PageCommand::Outdent { ids: vec![id] })?;
        } else {
            doc.apply(PageCommand::SetBlockKind {
                id,
                kind: BlockKind::Paragraph,
            })?;
        }
        return Ok(Caret { block: id, at: 0 });
    }
    // At the very start of a block with text, splitting would leave the empty half with the block's
    // kind and move the text into a paragraph, so a heading would lose its style (E17-S13). Instead
    // an empty paragraph goes above and the block stays as it is, caret and all. Lists already
    // continue in their own kind, so they split as before.
    if at == 0 && !text.is_empty() && !is_list {
        insert_before(doc, id, BlockKind::Paragraph)?;
        return Ok(Caret { block: id, at: 0 });
    }
    let PageOutcome::BlockCreated(block) = doc.apply(PageCommand::SplitBlock { id, pos: at })? else {
        unreachable!("SplitBlock always reports the block it created");
    };
    Ok(Caret { block, at: 0 })
}

/// Adds an empty block of `kind` right before `id`, at the same level.
pub(crate) fn insert_before(doc: &mut PageDoc, id: BlockId, kind: BlockKind) -> Result<BlockId, EditorError> {
    let parent = doc.parent(id)?;
    let siblings = match parent {
        Some(parent) => doc.children(parent)?,
        None => doc.root_blocks(),
    };
    let index = siblings.iter().position(|&b| b == id).unwrap_or(0);
    insert_block(doc, parent, index, kind)
}

/// Adds an empty block of `kind` right after `id`, at the same level.
pub(crate) fn insert_after(doc: &mut PageDoc, id: BlockId, kind: BlockKind) -> Result<BlockId, EditorError> {
    let parent = doc.parent(id)?;
    let siblings = match parent {
        Some(parent) => doc.children(parent)?,
        None => doc.root_blocks(),
    };
    let index = siblings
        .iter()
        .position(|&b| b == id)
        .map_or(siblings.len(), |p| p + 1);
    insert_block(doc, parent, index, kind)
}

pub(crate) fn insert_block(
    doc: &mut PageDoc,
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
    let PageOutcome::BlockCreated(id) = doc.apply(command)? else {
        unreachable!("InsertBlock always reports the block it created");
    };
    Ok(id)
}

/// Adds an empty `rows` × `columns` table under `parent` at `index` and returns it.
pub(crate) fn insert_table(
    doc: &mut PageDoc,
    parent: Option<BlockId>,
    index: usize,
    (rows, columns): (usize, usize),
    header_row: bool,
) -> Result<BlockId, EditorError> {
    let command = PageCommand::InsertTable {
        parent,
        index,
        rows,
        columns,
        header_row,
    };
    let PageOutcome::BlockCreated(id) = doc.apply(command)? else {
        unreachable!("InsertTable always reports the table it created");
    };
    Ok(id)
}

#[cfg(test)]
mod tests;
