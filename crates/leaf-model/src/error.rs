//! Errors from the document model.

use leaf_core::PageId;
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

    /// An icon was set to empty text; clearing an icon uses `None`.
    #[error("page {0}: icon cannot be empty (use None to clear it)")]
    EmptyIcon(PageId),

    /// Loro rejected an operation or could not read the bytes.
    #[error(transparent)]
    Loro(#[from] loro::LoroError),

    /// Loro could not export the document.
    #[error(transparent)]
    Export(#[from] loro::LoroEncodeError),
}
