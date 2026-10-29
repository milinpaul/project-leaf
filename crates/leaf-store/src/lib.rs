//! Persistence for Leaf workspaces: SQLite storage of Loro documents, autosave and crash recovery.
//!
//! Documents are saved as Loro snapshots plus small incremental updates, so typing is durable
//! within a second without rewriting whole pages. Isolating storage here keeps the model and
//! editor portable (they also run in WASM) and lets us swap or extend storage later, for
//! example for cloud sync.
//!
//! The database schema lives in [`migrations`]: a numbered, forward-only list, so every file
//! written by any version of Leaf can be brought up to date, and a file from a newer Leaf is
//! refused instead of being damaged.
//!
//! A workspace on disk is a folder ([`Workspace`]):
//!
//! ```text
//! MyWorkspace.leaf/
//!   workspace.sqlite   all documents (plus -wal/-shm files while open)
//!   assets/            images, named by content hash
//!   search/            the search index, rebuildable from the documents
//!   leaf.lock          locked by the app that has the workspace open (ADR 0009)
//! ```
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.

mod autosave;
mod docs;
mod error;
pub mod migrations;
mod store;
mod versions;
mod workspace;

pub use autosave::{Autosave, DEFAULT_AUTOSAVE_MS};
pub use docs::{DocKey, Document, Recovered, Savable};
pub use error::StoreError;
pub use migrations::SCHEMA_VERSION;
pub use store::Store;
pub use versions::SavedVersion;
pub use workspace::Workspace;
