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

    /// Loro rejected an operation or could not read the bytes.
    #[error(transparent)]
    Loro(#[from] loro::LoroError),

    /// Loro could not export the document.
    #[error(transparent)]
    Export(#[from] loro::LoroEncodeError),
}
