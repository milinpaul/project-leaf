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

/// Which save of a page the indexed rows came from, as `leaf_store::SavedVersion` reports it.
///
/// Stored with the rows, so the index can tell which pages changed since it last read them
/// without the editor reporting anything (ADR 0024).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PageVersion {
    /// When the page was last saved, in Unix milliseconds: ties in ranking go to the newer page.
    pub saved_at: i64,
    /// The store's sequence number for that save; only ever grows, so it detects every change.
    pub seq: i64,
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
