//! Commands that change a page's blocks. Like workspace commands, each is applied as exactly one
//! Loro commit, so each will be one undo step (E02-S08).

use std::ops::Range;

use leaf_core::BlockId;

use crate::{BlockKind, BlockProp, Mark, MarkKind};

/// A change to a page document.
///
/// Applied with [`PageDoc::apply`](crate::PageDoc::apply).
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
    /// Replaces the characters in `range` with `text` in one step (typing over a selection, or
    /// committing an input-method composition), so it undoes as one. The new text takes the formatting
    /// of the first character it replaces, as in word processors; with an empty range it is
    /// [`PageCommand::InsertText`].
    ReplaceText {
        /// Block whose text to edit; it must have text.
        id: BlockId,
        /// Unicode scalar positions, `start..end`, within the text.
        range: Range<usize>,
        /// The replacement; empty just deletes.
        text: String,
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
    /// Turns a block into another kind ("Turn into"). Text is kept when the new kind has text; a
    /// divider needs empty text, so no words are lost. Code drops all marks, and a kind that can't
    /// hold children lets its children out to sit right after it. Unknown kinds can't be converted.
    SetBlockKind {
        /// Block to convert.
        id: BlockId,
        /// What it becomes, with its properties.
        kind: BlockKind,
    },
    /// Changes one property of a block, such as ticking a to-do.
    SetProp {
        /// Block to change.
        id: BlockId,
        /// The property and its new value; it must belong to the block's kind.
        prop: BlockProp,
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
    /// Enter in the middle of text: the text after `pos` moves, with its formatting, into a new block
    /// right after this one (or its first child, if it has children). Lists and to-dos continue in
    /// the new block; anything else continues as a paragraph. Returns the new block.
    SplitBlock {
        /// Block to split; it must have text.
        id: BlockId,
        /// Unicode scalar position to split at, at most the text length.
        pos: usize,
    },
    /// "Duplicate" from the block menu: a deep copy of each block, with everything nested in it, right
    /// after the original. Everything stored is copied, formatting and properties this version doesn't
    /// know included; only the block IDs are new. Blocks inside another listed block are copied with it,
    /// not twice. Returns the copies of the outermost blocks, in reading order, in
    /// [`PageOutcome::BlocksCreated`].
    DuplicateBlocks {
        /// Blocks to copy.
        ids: Vec<BlockId>,
    },
    /// Backspace at the start of a block: its text, with formatting, is appended to the block just
    /// above it in reading order, its children move there too, and the block is removed. Reports
    /// where the two texts joined in [`PageOutcome::BlockMerged`]. The block above must have text:
    /// a divider or image is refused with `MergeTargetNotText`, so the editor can select it instead.
    MergeIntoPrevious {
        /// Block to merge away.
        id: BlockId,
    },
}

/// What applying a page command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageOutcome {
    /// A block was created with this ID.
    BlockCreated(BlockId),
    /// These blocks were created, in reading order (the outermost copies made by duplicating).
    BlocksCreated(Vec<BlockId>),
    /// These blocks were deleted: each listed block and everything inside it, in document order.
    BlocksDeleted(Vec<BlockId>),
    /// A block was merged into `into`; its text starts at position `at` there, which is where the
    /// cursor belongs.
    BlockMerged {
        /// The block the text was appended to.
        into: BlockId,
        /// Unicode scalar position of the join.
        at: usize,
    },
    /// The page changed, as exactly one undo step.
    Changed,
    /// The command was valid but changed nothing, so nothing was written.
    Unchanged,
}
