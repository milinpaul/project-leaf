//! Opening workspace databases: WAL, schema version and migrations (E03-S01).

mod common;

use std::path::{Path, PathBuf};

use leaf_store::{SCHEMA_VERSION, Store, StoreError};
use rusqlite::Connection;

use common::TempDir;

fn db(dir: &TempDir) -> PathBuf {
    dir.0.join("workspace.sqlite")
}

fn names(path: &Path, kind: &str) -> rusqlite::Result<Vec<String>> {
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    stmt.query_map([kind], |row| row.get(0))?.collect()
}

fn stored_version(path: &Path) -> rusqlite::Result<String> {
    Connection::open(path)?.query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |row| {
        row.get(0)
    })
}

#[test]
fn a_new_file_gets_the_whole_schema_and_the_current_version() {
    let dir = TempDir::new();
    let store = Store::open(&db(&dir)).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(
        names(&db(&dir), "table").unwrap(),
        vec!["assets", "doc_updates", "docs", "meta"]
    );
    assert_eq!(names(&db(&dir), "index").unwrap(), vec!["doc_updates_by_doc"]);
    assert_eq!(stored_version(&db(&dir)).unwrap(), "1");
}

#[test]
fn the_file_uses_write_ahead_logging_and_enforces_foreign_keys() {
    let dir = TempDir::new();
    let _store = Store::open(&db(&dir)).unwrap();
    // WAL is a property of the file, so a plain connection sees it too.
    let conn = Connection::open(db(&dir)).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    assert!(dir.0.join("workspace.sqlite-wal").exists());

    conn.pragma_update(None, "foreign_keys", true).unwrap();
    let orphan = conn.execute(
        "INSERT INTO doc_updates (doc_id, data, created_at) VALUES ('nope', x'00', 0)",
        [],
    );
    assert!(orphan.is_err(), "an update must belong to a saved document");
}

#[test]
fn reopening_keeps_data_and_runs_nothing_again() {
    let dir = TempDir::new();
    drop(Store::open(&db(&dir)).unwrap());
    Connection::open(db(&dir))
        .unwrap()
        .execute(
            "INSERT INTO docs (doc_id, snapshot, updated_at) VALUES ('w', x'01', 1)",
            [],
        )
        .unwrap();

    let store = Store::open(&db(&dir)).unwrap();
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    let count: i64 = Connection::open(db(&dir))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM docs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn a_file_from_a_newer_leaf_is_refused_and_left_untouched() {
    let dir = TempDir::new();
    drop(Store::open(&db(&dir)).unwrap());
    Connection::open(db(&dir))
        .unwrap()
        .execute_batch(
            "UPDATE meta SET value = '99' WHERE key = 'schema_version';
             CREATE TABLE from_the_future (x INTEGER);",
        )
        .unwrap();

    match Store::open(&db(&dir)) {
        Err(StoreError::SchemaTooNew { found, supported }) => {
            assert_eq!((found, supported), (99, SCHEMA_VERSION));
        }
        other => panic!("expected SchemaTooNew, got {other:?}"),
    }
    assert_eq!(stored_version(&db(&dir)).unwrap(), "99");
    assert!(
        names(&db(&dir), "table")
            .unwrap()
            .contains(&"from_the_future".to_owned())
    );
}

#[test]
fn a_database_without_write_ahead_logging_is_refused() {
    // SQLite's in-memory databases can't use WAL; autosave's guarantee depends on it.
    match Store::open(Path::new(":memory:")) {
        Err(StoreError::WalUnavailable(mode)) => assert_eq!(mode, "memory"),
        other => panic!("expected WalUnavailable, got {other:?}"),
    }
}

#[test]
fn errors_explain_themselves() {
    let too_new = StoreError::SchemaTooNew {
        found: 3,
        supported: 1,
    };
    assert_eq!(
        too_new.to_string(),
        "workspace schema version 3 is newer than this app supports (1)"
    );
    assert_eq!(
        StoreError::BadSchemaVersion("x".into()).to_string(),
        "workspace schema version \"x\" is not a number"
    );
}
