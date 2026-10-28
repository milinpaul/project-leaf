//! Tests for the migration runner itself, with made-up migration lists.

use rusqlite::Connection;

use super::{MIGRATIONS, run, version};
use crate::StoreError;

const CREATE_META: &str = "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);";

fn tables(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn an_empty_database_is_version_zero_and_migrates_step_by_step() {
    let mut conn = Connection::open_in_memory().unwrap();
    assert_eq!(version(&conn).unwrap(), 0);
    run(&mut conn, &[CREATE_META]).unwrap();
    assert_eq!(version(&conn).unwrap(), 1);

    run(&mut conn, &[CREATE_META, "CREATE TABLE later (x INTEGER);"]).unwrap();
    assert_eq!(version(&conn).unwrap(), 2, "only the new migration ran");
    assert_eq!(tables(&conn), vec!["later", "meta"]);
}

#[test]
fn a_failing_migration_rolls_back_with_its_version() {
    let mut conn = Connection::open_in_memory().unwrap();
    run(&mut conn, &[CREATE_META]).unwrap();
    let broken = [CREATE_META, "CREATE TABLE half (x INTEGER); NOT VALID SQL;"];
    assert!(matches!(run(&mut conn, &broken), Err(StoreError::Sqlite(_))));
    assert_eq!(version(&conn).unwrap(), 1);
    assert_eq!(tables(&conn), vec!["meta"], "the half-done table is gone");
}

#[test]
fn a_meta_table_without_a_version_counts_as_zero() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(CREATE_META).unwrap();
    assert_eq!(version(&conn).unwrap(), 0);
}

#[test]
fn a_version_that_is_not_a_number_is_refused() {
    let mut conn = Connection::open_in_memory().unwrap();
    run(&mut conn, MIGRATIONS).unwrap();
    conn.execute("UPDATE meta SET value = 'two' WHERE key = 'schema_version'", [])
        .unwrap();
    assert!(matches!(
        run(&mut conn, MIGRATIONS),
        Err(StoreError::BadSchemaVersion(v)) if v == "two"
    ));
}
