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
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.

mod error;
pub mod migrations;
mod store;

pub use error::StoreError;
pub use migrations::SCHEMA_VERSION;
pub use store::Store;
