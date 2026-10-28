//! Commands that change a page's blocks. Like workspace commands, each is applied as exactly one
//! Loro commit, so each will be one undo step (E02-S08).

use std::ops::Range;

use leaf_core::BlockId;

use crate::{BlockKind, Mark, MarkKind};

/// A change to a page document.
///
/// Applied with [`PageDoc::apply`](crate::PageDoc::apply). Kind changes and split/merge arrive in E02-S04
/// and E02-S05.
#[derive(Debug, Clone, PartialEq)]
pub enum PageCommand {
    /// Adds a block under `parent` (or at the top level when `None`) at position `index`, and returns
    /// its new ID in [`PageOutcome::BlockCreated`].
    InsertBlock {
        /// Block to nest under; its kind must allow children.
        parent: Option<BlockId>,
        /// Position among the parent's children; equal to the child count appends.
        index: usize,
        /// What kind of block to create. Unknown kinds can't be created.
        kind: BlockKind,
        /// Initial text, plain. Must be empty for kinds without text (dividers).
        text: String,
    },
    /// Deletes blocks together with everything nested inside them. Blocks already inside another
    /// listed block are fine; an empty list changes nothing.
    DeleteBlocks {
        /// Blocks to delete.
        ids: Vec<BlockId>,
    },
    /// Moves blocks, with what is nested inside them, under `new_parent` at `index`. The moved
    /// blocks keep their document order, whatever order they are listed in; a listed block inside
    /// another listed block simply travels with it.
    MoveBlocks {
        /// Blocks to move.
        ids: Vec<BlockId>,
        /// Block to move them into (its kind must allow children), or `None` for the top level.
        new_parent: Option<BlockId>,
        /// Final position of the first moved block among the destination's other children, counted
        /// without the moved blocks themselves.
        index: usize,
    },
    /// Inserts plain text at `pos`. Inserted text picks up marks according to the expand table
    /// (typing right after bold stays bold; after a link it doesn't).
    InsertText {
        /// Block whose text to edit; it must have text.
        id: BlockId,
        /// Position in Unicode scalar values (not bytes, not graphemes), at most the text length.
        pos: usize,
        /// Text to insert; empty changes nothing.
        text: String,
    },
    /// Deletes the characters in `range`.
    DeleteText {
        /// Block whose text to edit; it must have text.
        id: BlockId,
        /// Unicode scalar positions, `start..end`, within the text; empty changes nothing.
        range: Range<usize>,
    },
    /// Applies a mark to the characters in `range`. Code blocks take no marks.
    SetMark {
        /// Block whose text to format.
        id: BlockId,
        /// Unicode scalar positions within the text; empty changes nothing.
        range: Range<usize>,
        /// The mark, with its link address if it is a link.
        mark: Mark,
    },
    /// Removes a mark from the characters in `range`.
    RemoveMark {
        /// Block whose text to format.
        id: BlockId,
        /// Unicode scalar positions within the text; empty changes nothing.
        range: Range<usize>,
        /// Which mark to remove.
        kind: MarkKind,
    },
    /// Tab: each block becomes the last child of the block just above it. Blocks with nothing above
    /// them, or whose block above can't hold children, stay put.
    Indent {
        /// Blocks to indent; handled one by one in reading order, nested ones travel with their parent.
        ids: Vec<BlockId>,
    },
    /// Shift+Tab: each block moves out to sit right after its parent, and the blocks that followed it
    /// become its children. Top-level blocks stay put, and so does a block that can't hold children
    /// while blocks follow it, since moving them elsewhere would reorder the text.
    Outdent {
        /// Blocks to outdent; handled one by one in reading order, nested ones travel with their parent.
        ids: Vec<BlockId>,
    },
}

/// What applying a page command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageOutcome {
    /// A block was created with this ID.
    BlockCreated(BlockId),
    /// These blocks were deleted: each listed block and everything inside it, in document order.
    BlocksDeleted(Vec<BlockId>),
    /// The page changed, as exactly one undo step.
    Changed,
    /// The command was valid but changed nothing, so nothing was written.
    Unchanged,
}
