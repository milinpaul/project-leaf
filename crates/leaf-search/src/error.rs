//! Errors from the search index.

use thiserror::Error;

/// Everything that can go wrong opening or writing the search index.
///
/// None of these can lose user data: the index is rebuilt from the documents, so callers log the
/// error and rebuild rather than stopping the owner from editing.
#[derive(Debug, Error)]
pub enum SearchError {
    /// SQLite failed (disk full, damaged index file, ...).
    #[error("search index database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Creating the `search/` folder or deleting a damaged index file failed.
    #[error("search index file system error: {0}")]
    Io(#[from] std::io::Error),

    /// A page couldn't be read while extracting its text (a block or page missing mid-read).
    #[error("search could not read a page: {0}")]
    Model(#[from] leaf_model::ModelError),
}
