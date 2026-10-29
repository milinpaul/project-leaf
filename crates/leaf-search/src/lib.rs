//! Full-text search across a Leaf workspace, using SQLite's built-in FTS5 (ADR 0019).
//!
//! The search index is disposable: it can always be rebuilt from the documents, so it never
//! holds data that exists nowhere else. It lives in its own file, `search/index.sqlite`, never in
//! `workspace.sqlite`, so it can be deleted and rebuilt at any time and needs no schema
//! migrations, only an [`INDEX_VERSION`]. Keeping it in a separate crate keeps indexing out of the
//! model, the editor and the WASM build.
//!
//! The index takes plain rows ([`PageText`]) rather than documents, so this crate knows nothing
//! about how pages are stored or edited; the caller extracts the text and hands it over.
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.

mod error;
mod index;
mod page_text;
mod schema;

pub use error::SearchError;
pub use index::{INDEX_FILE, IndexState, SEARCH_DIR, SearchIndex};
pub use page_text::{BlockText, PageText};
pub use schema::INDEX_VERSION;
