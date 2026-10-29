//! Errors from the storage layer.

use std::path::PathBuf;

use thiserror::Error;

/// Everything that can go wrong opening or using a workspace database.
#[derive(Debug, Error)]
pub enum StoreError {
    /// SQLite itself failed (file unreadable, disk full, corrupt database, ...).
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The file was written by a newer Leaf. It is left untouched rather than opened, because
    /// this version doesn't know the newer schema and could damage it.
    #[error("workspace schema version {found} is newer than this app supports ({supported})")]
    SchemaTooNew {
        /// The version recorded in the file.
        found: u32,
        /// The newest version this app knows.
        supported: u32,
    },

    /// `meta.schema_version` holds something that isn't a version number, so it is unclear which
    /// migrations have run. Opening stops rather than guessing.
    #[error("workspace schema version {0:?} is not a number")]
    BadSchemaVersion(String),

    /// SQLite would not switch the file to write-ahead logging (for example an in-memory
    /// database). Autosave relies on WAL for its crash-safety guarantee, so this is an error.
    #[error("write-ahead logging is unavailable (journal mode is {0:?})")]
    WalUnavailable(String),

    /// Reading or writing the workspace folder failed (permissions, disk full, ...).
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),

    /// A new workspace was asked for where something already exists. Creating refuses rather
    /// than mixing a workspace into someone's files.
    #[error("{} already exists and is not an empty folder", .0.display())]
    AlreadyExists(PathBuf),

    /// The path isn't a Leaf workspace folder (missing, not a folder, or no `workspace.sqlite`).
    #[error("{} is not a Leaf workspace", .0.display())]
    NotAWorkspace(PathBuf),

    /// Another Leaf window or process already has this workspace open. Two writers would
    /// each autosave over the other's edits, so the second is refused.
    #[error("{} is already open in another Leaf window", .0.display())]
    AlreadyOpen(PathBuf),

    /// A file offered as an image isn't one Leaf recognises (PNG, JPEG, GIF, WebP, BMP, TIFF, AVIF,
    /// HEIC or SVG). Its type is read from its content, not its name.
    #[error("not an image Leaf can store")]
    NotAnImage,

    /// Saved bytes couldn't be turned back into a document (or a document into bytes).
    #[error("document error: {0}")]
    Model(#[from] leaf_model::ModelError),
}
