//! The rows the index stores for one page.

use leaf_core::{BlockId, PageId};

/// The searchable text of one page: what [`crate::SearchIndex::index_page`] stores.
///
/// Plain data rather than a document, so the index never depends on how pages are stored or
/// edited. Marks are already stripped; only words matter for search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageText {
    /// The page these rows belong to.
    pub page: PageId,
    /// The page title, searched separately so title matches can rank above body matches.
    pub title: String,
    /// One entry per block that has text, in document order.
    pub blocks: Vec<BlockText>,
}

/// The plain text of one block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockText {
    /// The block, so a result can open the page scrolled to it.
    pub block: BlockId,
    /// The block's kind name (`paragraph`, `code`, ...), so results can show or filter by kind.
    /// A string rather than a model type so unknown kinds pass through untouched.
    pub kind: String,
    /// The block's text without marks.
    pub text: String,
}
