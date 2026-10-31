//! Saving, loading and recovering a database document, like a page (E09-S03).
#![allow(clippy::panic, reason = "test helpers fail a test by panicking")]

mod common;

use common::TempDir;
use leaf_core::{DatabaseId, PageId};
use leaf_model::{CellValue, Database, DatabaseCommand, DatabaseOutcome, PropertyType};
use leaf_store::{DocKey, Workspace as Folder};
use rusqlite::Connection;

/// A database with a "Name" column and `rows` rows named "row 0", "row 1", ...
fn database(rows: usize) -> (Database, Vec<PageId>) {
    let mut db = Database::new();
    let Ok(DatabaseOutcome::ColumnCreated(name)) = db.apply(DatabaseCommand::AddColumn {
        index: 0,
        name: "Name".into(),
        kind: PropertyType::Text,
    }) else {
        panic!("expected a column");
    };
    let ids = (0..rows)
        .map(|i| {
            let row = PageId::new();
            assert!(db.apply(DatabaseCommand::AddRow { row, index: i }).is_ok());
            let value = Some(CellValue::Text(format!("row {i}")));
            assert!(
                db.apply(DatabaseCommand::SetCell {
                    row,
                    column: name,
                    value,
                })
                .is_ok()
            );
            row
        })
        .collect();
    (db, ids)
}

#[test]
fn a_database_survives_save_and_reopen_and_lists_under_its_own_key() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let key = DocKey::Database(DatabaseId::new());
    let (mut db, rows) = database(2);
    let mut folder = Folder::create(&path).unwrap();
    assert!(folder.store_mut().save(key, &db, 1).unwrap(), "snapshot");
    db.apply(DatabaseCommand::DeleteRow { row: rows[0] }).unwrap();
    assert!(folder.store_mut().save(key, &db, 2).unwrap(), "one update");
    assert!(!folder.store_mut().save(key, &db, 3).unwrap(), "nothing new");
    assert!(key.to_string().starts_with("database:"), "{key}");
    let versions = folder.store().saved_versions().unwrap();
    assert!(versions.contains_key(&key), "read back as a database key");
    drop(folder);

    let folder = Folder::open(&path).unwrap();
    let loaded: Database = folder.store().load(key).unwrap().unwrap();
    assert_eq!(loaded.rows(), [rows[1]]);
    assert!(!loaded.has_unsaved_changes());
    let missing: Option<Database> = folder.store().load(DocKey::Database(DatabaseId::new())).unwrap();
    assert!(missing.is_none());
}

#[test]
fn a_damaged_update_of_a_database_is_skipped_and_the_rest_recovers() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let key = DocKey::Database(DatabaseId::new());
    let (mut db, rows) = database(1);
    let mut folder = Folder::create(&path).unwrap();
    folder.store_mut().save(key, &db, 1).unwrap();
    db.apply(DatabaseCommand::MoveRow {
        row: rows[0],
        index: 0,
    })
    .unwrap();
    let extra = PageId::new();
    db.apply(DatabaseCommand::AddRow { row: extra, index: 1 })
        .unwrap();
    folder.store_mut().save(key, &db, 2).unwrap();
    db.apply(DatabaseCommand::DeleteRow { row: extra }).unwrap();
    folder.store_mut().save(key, &db, 3).unwrap();
    drop(folder);
    Connection::open(path.join("workspace.sqlite"))
        .unwrap()
        .execute(
            "UPDATE doc_updates SET data = x'00' WHERE seq = (SELECT MAX(seq) FROM doc_updates WHERE doc_id = ?1)",
            [key.to_string()],
        )
        .unwrap();

    let folder = Folder::open(&path).unwrap();
    // Like a page: loading warns and carries on without the damaged update, which stays on disk.
    let loaded: Database = folder.store().load(key).unwrap().unwrap();
    assert_eq!(loaded.rows(), [rows[0], extra]);
    let recovered = folder.store().recover::<Database>(key).unwrap().unwrap();
    assert_eq!(recovered.skipped.len(), 1);
    assert_eq!(
        recovered.doc.rows(),
        [rows[0], extra],
        "everything before the damaged update"
    );
}
