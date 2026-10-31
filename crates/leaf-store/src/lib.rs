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
//! App-wide settings (window size, sidebar width) live outside any workspace, in [`settings`].
//!
//! A workspace on disk is a folder ([`Workspace`]):
//!
//! ```text
//! MyWorkspace.leaf/
//!   workspace.sqlite   all documents (plus -wal/-shm files while open)
//!   assets/            images, named by content hash ([`Workspace::import_asset`])
//!   search/            the search index, rebuildable from the documents
//!   leaf.lock          locked by the app that has the workspace open (ADR-009)
//!   view.json          how the sidebar was left on this computer (expanded pages); not a document
//! ```
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.

// Core crates must not panic on bad input, so indexing and arithmetic are checked here (E08-S15).
// Unit tests are exempt, like the workspace exempts them from unwrap/expect.
#![cfg_attr(not(test), warn(clippy::indexing_slicing, clippy::arithmetic_side_effects))]

mod assets;
mod autosave;
mod compaction;
mod docs;
mod error;
mod json_file;
pub mod migrations;
pub mod settings;
mod store;
mod versions;
mod view_state;
mod workspace;

pub use assets::{Asset, SvgProblem, check_svg, image_mime};
pub use autosave::{Autosave, DEFAULT_AUTOSAVE_MS};
pub use compaction::COMPACT_AFTER;
pub use docs::{DocKey, Document, Recovered, Savable};
pub use error::StoreError;
pub use migrations::SCHEMA_VERSION;
pub use settings::{AppSettings, Appearance, DisplayArea, RecentWorkspace, WindowFrame};
pub use store::Store;
pub use versions::SavedVersion;
pub use view_state::{VIEW_STATE, ViewState};
pub use workspace::{FolderKind, NameProblem, Workspace, inspect, new_workspace_path};
