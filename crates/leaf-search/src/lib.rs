//! Full-text search across a Leaf workspace, using SQLite's built-in FTS5 (ADR 0019).
//!
//! The search index is disposable: it can always be rebuilt from the documents, so it never
//! holds data that exists nowhere else. It lives in its own file, `search/index.sqlite`, never in
//! `workspace.sqlite`, so it can be deleted and rebuilt at any time and needs no schema
//! migrations, only an [`INDEX_VERSION`]. Keeping it in a separate crate keeps indexing out of the
//! model, the editor and the WASM build.
//!
//! The index stores plain rows ([`PageText`]) rather than documents. [`page_text`] reads them out of a
//! page, read-only, so indexing can never change or lose anything in a document.
//!
//! - [`SearchWorker`] keeps the index current on its own thread: after each save it compares what is
//!   saved with what it indexed ([`Syncer`], ADR 0024) and re-reads only what changed, through a
//!   [`PageSource`] the app supplies.
//! - [`Searcher`] searches it, on a separate read-only connection: every typed word as a prefix,
//!   ignoring case and accents, title matches first, with highlighted excerpts ([`PageHit`]).
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`; the store is reached through [`PageSource`].

mod error;
mod extract;
mod index;
mod page_text;
mod query;
mod schema;
mod search;
mod source;
mod sync;
mod worker;

pub use error::SearchError;
pub use extract::{UNTITLED, page_text};
pub use index::{INDEX_FILE, IndexState, IndexedPage, SEARCH_DIR, SearchIndex};
pub use page_text::{BlockText, PageText, PageVersion};
pub use schema::INDEX_VERSION;
pub use search::{BLOCKS_PER_PAGE, BlockHit, Highlighted, MAX_PAGES, PageHit, RANKED_BLOCKS, Searcher};
pub use source::{PageSource, SavedVersions, SourceError};
pub use sync::{Finished, Syncer};
pub use worker::SearchWorker;
