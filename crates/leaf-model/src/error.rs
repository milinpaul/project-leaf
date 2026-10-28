//! Errors from the document model.

use leaf_core::{BlockId, PageId};
use thiserror::Error;

/// Everything that can go wrong reading or changing Leaf documents.
///
/// Loro's two error types are wrapped rather than exposed piecemeal, so callers (store, editor)
/// handle one type.
#[derive(Debug, Error)]
pub enum ModelError {
    /// The page is not in this workspace (never existed, or permanently deleted).
    #[error("page {0} not found")]
    PageNotFound(PageId),

    /// A page node in the saved workspace has no usable identity, so it cannot be addressed.
    /// Loading fails rather than hiding the page, so no data is silently lost.
    #[error("invalid page node {node}: {reason}")]
    InvalidPage {
        /// Loro's internal node ID, for debugging only.
        node: String,
        /// What was wrong with it.
        reason: String,
    },

    /// Two page nodes claim the same `PageId`.
    #[error("page {0} appears twice in the workspace")]
    DuplicatePage(PageId),

    /// A page's metadata field is missing or has the wrong type.
    #[error("page {page}: field {field:?} {reason}")]
    InvalidPageMeta {
        /// The page whose metadata is wrong.
        page: PageId,
        /// The metadata key, for example `"title"`.
        field: &'static str,
        /// What was wrong with it.
        reason: String,
    },

    /// A position is past the end of a list of siblings.
    #[error("index {index} is past the end ({len} items)")]
    IndexOutOfRange {
        /// The position asked for.
        index: usize,
        /// How many items there are; `index` may be at most this.
        len: usize,
    },

    /// A page cannot be moved into itself or into a page nested inside it: that would cut the
    /// branch it sits on off from the tree.
    #[error("cannot move page {page} into {new_parent}, which is the page itself or inside it")]
    MoveIntoOwnSubtree {
        /// The page being moved.
        page: PageId,
        /// The parent that was asked for.
        new_parent: PageId,
    },

    /// The page is in the trash (or inside a trashed page), so it can't be trashed again, moved,
    /// or given new pages. Restore it first.
    #[error("page {0} is in the trash")]
    PageInTrash(PageId),

    /// Restore and permanent delete only apply to pages in the trash.
    #[error("page {0} is not in the trash")]
    NotInTrash(PageId),

    /// An icon was set to empty text; clearing an icon uses `None`.
    #[error("page {0}: icon cannot be empty (use None to clear it)")]
    EmptyIcon(PageId),

    /// The block is not on this page (never existed, or deleted).
    #[error("block {0} not found")]
    BlockNotFound(BlockId),

    /// A block node in the saved page has no usable identity. Loading fails rather than hiding it.
    #[error("invalid block node {node}: {reason}")]
    InvalidBlock {
        /// Loro's internal node ID, for debugging only.
        node: String,
        /// What was wrong with it.
        reason: String,
    },

    /// Two block nodes claim the same `BlockId`.
    #[error("block {0} appears twice on the page")]
    DuplicateBlock(BlockId),

    /// A block's type or properties are missing or have the wrong type.
    #[error("block {block}: {field:?} {reason}")]
    InvalidBlockData {
        /// The block whose data is wrong.
        block: BlockId,
        /// The meta or props key, for example `"level"`.
        field: &'static str,
        /// What was wrong with it.
        reason: String,
    },

    /// The block kind can't be created as given (unknown type, or a heading level outside 1 to 3).
    #[error("invalid block kind: {0}")]
    InvalidKind(String),

    /// This kind of block has no text (dividers), so it can't be given any.
    #[error("{kind} blocks have no text")]
    TextNotAllowed {
        /// The block type.
        kind: String,
    },

    /// This kind of block can't contain other blocks (headings, code, dividers, images, unknown).
    #[error("block {parent} is a {kind}, which can't contain other blocks")]
    ChildrenNotAllowed {
        /// The block asked to be the parent.
        parent: BlockId,
        /// Its type.
        kind: String,
    },

    /// A block can't be moved into itself or into a block nested inside it.
    #[error("cannot move block {block} into {new_parent}, which is the block itself or inside it")]
    BlockMoveIntoOwnSubtree {
        /// The moved block whose subtree contains the destination.
        block: BlockId,
        /// The parent that was asked for.
        new_parent: BlockId,
    },

    /// Loro rejected an operation or could not read the bytes.
    #[error(transparent)]
    Loro(#[from] loro::LoroError),

    /// Loro could not export the document.
    #[error(transparent)]
    Export(#[from] loro::LoroEncodeError),
}
