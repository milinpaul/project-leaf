//! The database schema and how files are brought up to date.
//!
//! [`MIGRATIONS`] is a forward-only list: entry `n` turns a version-`n` file into version `n + 1`
//! (an empty file is version 0). Released entries are never edited or removed; a schema change is
//! always a new entry at the end. The version is stored in `meta.schema_version` and bumped in the
//! same transaction as the migration, so a failed migration leaves the file exactly as it was.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::StoreError;

/// Version 1, the schema in `docs/architecture/BLOCK_MODEL.md` ("Persistence"):
/// - `docs`: one Loro snapshot per document (the workspace and each page).
/// - `doc_updates`: small Loro updates saved since that snapshot, replayed in `seq` order on load
///   and folded into the snapshot by compaction.
/// - `assets`: what is known about each image file stored in `assets/` by its content hash.
/// - `meta`: workspace-wide settings, including `schema_version`.
const V1: &str = "
CREATE TABLE docs (
    doc_id     TEXT PRIMARY KEY,
    snapshot   BLOB NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE doc_updates (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    doc_id     TEXT NOT NULL REFERENCES docs(doc_id),
    data       BLOB NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX doc_updates_by_doc ON doc_updates(doc_id, seq);
CREATE TABLE assets (
    asset_id   TEXT PRIMARY KEY,
    mime       TEXT NOT NULL,
    bytes      INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";

/// Every migration, oldest first. Append only.
pub const MIGRATIONS: &[&str] = &[V1];

/// The schema version this build of Leaf writes and understands.
#[allow(clippy::cast_possible_truncation, reason = "a handful of migrations")]
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// The schema version recorded in the database; 0 for a new, empty file.
pub(crate) fn version(conn: &Connection) -> Result<u32, StoreError> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
        [],
        |row| row.get(0),
    )?;
    if !has_meta {
        return Ok(0);
    }
    let value: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |row| {
            row.get(0)
        })
        .optional()?;
    match value {
        None => Ok(0),
        Some(text) => text.parse().map_err(|_| StoreError::BadSchemaVersion(text)),
    }
}

/// Applies every migration the database hasn't had yet, each in its own transaction.
///
/// Each step re-reads the version inside an immediate (write-locked) transaction, so two
/// processes opening the same new file can't both run a migration.
pub(crate) fn run(conn: &mut Connection, migrations: &[&str]) -> Result<(), StoreError> {
    let supported = u32::try_from(migrations.len()).unwrap_or(u32::MAX);
    loop {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let found = version(&tx)?;
        if found > supported {
            return Err(StoreError::SchemaTooNew { found, supported });
        }
        let Some(sql) = usize::try_from(found).ok().and_then(|n| migrations.get(n)) else {
            return Ok(()); // up to date; dropping `tx` rolls back the empty transaction
        };
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [(found + 1).to_string()],
        )?;
        tx.commit()?;
    }
}

#[cfg(test)]
mod tests;
