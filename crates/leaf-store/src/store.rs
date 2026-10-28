//! The open workspace database.

use std::path::Path;

use rusqlite::Connection;

use crate::{StoreError, migrations};

/// A workspace's SQLite database, open and migrated to the current schema.
///
/// Opening sets the connection up the way autosave needs it:
/// - **WAL journal:** a committed save is never lost to an app crash, and saving doesn't block reads.
/// - **`synchronous = NORMAL`:** with WAL this survives app crashes; only a power cut can lose the
///   last moments of typing, which the product spec accepts in exchange for fast saves.
/// - **Foreign keys on:** SQLite leaves them off by default; on, an update can't point at a
///   document that doesn't exist.
#[derive(Debug)]
pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    /// Opens the database at `path`, creating it if missing, and brings its schema up to date.
    ///
    /// Fails with [`StoreError::SchemaTooNew`] for a file from a newer Leaf, leaving it untouched.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StoreError::WalUnavailable(mode));
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::run(&mut conn, migrations::MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// The schema version of the open database; always [`crate::SCHEMA_VERSION`] after `open`.
    pub fn schema_version(&self) -> Result<u32, StoreError> {
        migrations::version(&self.conn)
    }
}
