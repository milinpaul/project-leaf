//! Commands that change a page's blocks. Like workspace commands, each is applied as exactly one
//! Loro commit, so each will be one undo step (E02-S08).

use leaf_core::BlockId;

use crate::BlockKind;

/// A change to a page document.
///
/// Applied with [`PageDoc::apply`](crate::PageDoc::apply). Text, marks, kind changes, split/merge and
/// indenting arrive in E02-S03 to E02-S06.
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
