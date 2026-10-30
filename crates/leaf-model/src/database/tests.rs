//! Tests for the database document (E09-S02): every command, saving and loading, undo, merging two copies.

mod cells;
mod merges;

use leaf_core::{ColumnId, PageId};

use super::{Database, DatabaseCommand, DatabaseOutcome, PropertyType};
use crate::ModelError;

/// An empty database whose every command is its own undo step.
pub(super) fn database() -> Database {
    let mut db = Database::new();
    db.set_undo_merge_interval(0);
    db
}

pub(super) fn add_column(db: &mut Database, name: &str, kind: PropertyType) -> ColumnId {
    let index = db.column_ids().len();
    match db.apply(DatabaseCommand::AddColumn {
        index,
        name: name.into(),
        kind,
    }) {
        Ok(DatabaseOutcome::ColumnCreated(id)) => id,
        other => panic!("expected a column, got {other:?}"),
    }
}

pub(super) fn add_row(db: &mut Database) -> PageId {
    let row = PageId::new();
    let index = db.rows().len();
    db.apply(DatabaseCommand::AddRow { row, index }).unwrap();
    row
}

pub(super) fn names(db: &Database) -> Vec<String> {
    db.columns().unwrap().into_iter().map(|c| c.name).collect()
}

pub(super) fn checked(db: &Database) {
    assert_eq!(db.check_invariants(), Ok(()));
}

#[test]
fn columns_are_added_renamed_retyped_moved_hidden_sized_and_deleted() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let price = add_column(&mut db, "Price", PropertyType::Number);
    let first = db
        .apply(DatabaseCommand::AddColumn {
            index: 0,
            name: "First".into(),
            kind: PropertyType::Checkbox,
        })
        .unwrap();
    assert!(matches!(first, DatabaseOutcome::ColumnCreated(_)));
    assert_eq!(names(&db), ["First", "Name", "Price"]);

    db.apply(DatabaseCommand::RenameColumn {
        column: price,
        name: "Cost".into(),
    })
    .unwrap();
    db.apply(DatabaseCommand::SetColumnType {
        column: price,
        kind: PropertyType::Text,
    })
    .unwrap();
    db.apply(DatabaseCommand::MoveColumn {
        column: name,
        index: 2,
    })
    .unwrap();
    db.apply(DatabaseCommand::SetColumnHidden {
        column: price,
        hidden: true,
    })
    .unwrap();
    db.apply(DatabaseCommand::SetColumnWidth {
        column: price,
        width: Some(240.0),
    })
    .unwrap();
    assert_eq!(names(&db), ["First", "Cost", "Name"]);
    let cost = db.column(price).unwrap();
    assert_eq!(cost.kind, PropertyType::Text);
    assert!(cost.hidden);
    assert_eq!(cost.width, Some(240.0));
    db.apply(DatabaseCommand::SetColumnWidth {
        column: price,
        width: None,
    })
    .unwrap();
    assert_eq!(db.column(price).unwrap().width, None);

    db.apply(DatabaseCommand::DeleteColumn { column: price }).unwrap();
    assert_eq!(names(&db), ["First", "Name"]);
    assert!(matches!(db.column(price), Err(ModelError::ColumnNotFound(_))));
    checked(&db);
}

#[test]
fn refused_column_commands_change_nothing() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let before = db.to_bytes().unwrap();
    let refusals = [
        DatabaseCommand::AddColumn {
            index: 5,
            name: "x".into(),
            kind: PropertyType::Text,
        },
        DatabaseCommand::AddColumn {
            index: 0,
            name: "x".into(),
            kind: PropertyType::Unknown("formula".into()),
        },
        DatabaseCommand::SetColumnType {
            column: name,
            kind: PropertyType::Unknown("formula".into()),
        },
        DatabaseCommand::MoveColumn {
            column: name,
            index: 1,
        },
        DatabaseCommand::SetColumnWidth {
            column: name,
            width: Some(f64::NAN),
        },
        DatabaseCommand::RenameColumn {
            column: ColumnId::new(),
            name: "x".into(),
        },
        DatabaseCommand::AddOption {
            column: name,
            name: "x".into(),
            color: "red".into(),
        },
    ];
    for command in refusals {
        assert!(db.apply(command.clone()).is_err(), "{command:?}");
    }
    assert_eq!(db.to_bytes().unwrap(), before);
    // The same name again is valid and changes nothing, and says so.
    assert_eq!(
        db.apply(DatabaseCommand::RenameColumn {
            column: name,
            name: "Name".into(),
        })
        .unwrap(),
        DatabaseOutcome::Unchanged
    );
}

#[test]
fn rows_are_added_moved_and_deleted_and_a_page_is_one_row_once() {
    let mut db = database();
    let a = add_row(&mut db);
    let b = add_row(&mut db);
    let c = PageId::new();
    db.apply(DatabaseCommand::AddRow { row: c, index: 0 }).unwrap();
    assert_eq!(db.rows(), [c, a, b]);
    assert!(matches!(
        db.apply(DatabaseCommand::AddRow { row: a, index: 0 }),
        Err(ModelError::DuplicateRow(_))
    ));
    db.apply(DatabaseCommand::MoveRow { row: c, index: 2 }).unwrap();
    assert_eq!(db.rows(), [a, b, c]);
    assert_eq!(
        db.apply(DatabaseCommand::MoveRow { row: c, index: 2 }).unwrap(),
        DatabaseOutcome::Unchanged
    );
    db.apply(DatabaseCommand::DeleteRow { row: b }).unwrap();
    assert_eq!(db.rows(), [a, c]);
    assert!(!db.contains_row(b));
    assert!(matches!(
        db.apply(DatabaseCommand::DeleteRow { row: b }),
        Err(ModelError::RowNotFound(_))
    ));
    checked(&db);
}

#[test]
fn select_options_are_added_renamed_recoloured_and_deleted() {
    let mut db = database();
    let status = add_column(&mut db, "Status", PropertyType::Select);
    let DatabaseOutcome::OptionCreated(todo) = db
        .apply(DatabaseCommand::AddOption {
            column: status,
            name: "To do".into(),
            color: "gray".into(),
        })
        .unwrap()
    else {
        panic!("expected an option");
    };
    let DatabaseOutcome::OptionCreated(done) = db
        .apply(DatabaseCommand::AddOption {
            column: status,
            name: "Done".into(),
            color: "green".into(),
        })
        .unwrap()
    else {
        panic!("expected an option");
    };
    db.apply(DatabaseCommand::RenameOption {
        column: status,
        option: todo,
        name: "Later".into(),
    })
    .unwrap();
    db.apply(DatabaseCommand::SetOptionColor {
        column: status,
        option: done,
        color: "blue".into(),
    })
    .unwrap();
    let options = db.column(status).unwrap().options;
    let read: Vec<(&str, &str)> = options
        .iter()
        .map(|o| (o.name.as_str(), o.color.as_str()))
        .collect();
    assert_eq!(read, [("Later", "gray"), ("Done", "blue")]);
    db.apply(DatabaseCommand::DeleteOption {
        column: status,
        option: todo,
    })
    .unwrap();
    assert_eq!(db.column(status).unwrap().options.len(), 1);
    assert!(matches!(
        db.apply(DatabaseCommand::DeleteOption {
            column: status,
            option: todo,
        }),
        Err(ModelError::OptionNotFound { .. })
    ));
    checked(&db);
}

#[test]
fn a_database_survives_saving_and_loading_and_counts_as_saved() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let row = add_row(&mut db);
    db.apply(DatabaseCommand::SetCell {
        row,
        column: name,
        value: Some(super::CellValue::Text("Lamp".into())),
    })
    .unwrap();
    assert!(db.has_unsaved_changes());
    let snapshot = db.to_bytes().unwrap();
    db.mark_saved();
    assert!(!db.has_unsaved_changes());
    assert_eq!(db.unsaved_changes().unwrap(), None);

    db.apply(DatabaseCommand::RenameColumn {
        column: name,
        name: "Title".into(),
    })
    .unwrap();
    let update = db.unsaved_changes().unwrap().unwrap();
    let loaded = Database::from_saved(&snapshot, std::slice::from_ref(&update)).unwrap();
    assert!(!loaded.has_unsaved_changes());
    assert_eq!(names(&loaded), ["Title"]);
    assert_eq!(
        loaded.cell(row, name).unwrap(),
        Some(super::CellValue::Text("Lamp".into()))
    );
    let (recovered, skipped) = Database::recover(&snapshot, &[vec![1, 2, 3], update]).unwrap();
    assert_eq!(skipped, [0]);
    assert_eq!(names(&recovered), ["Title"]);
    assert!(Database::from_bytes(b"not a database").is_err());
}

#[test]
fn undo_and_redo_restore_a_deleted_column_with_its_cells_and_a_deleted_row() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let row = add_row(&mut db);
    db.apply(DatabaseCommand::SetCell {
        row,
        column: name,
        value: Some(super::CellValue::Text("Lamp".into())),
    })
    .unwrap();
    db.apply(DatabaseCommand::DeleteColumn { column: name }).unwrap();
    db.apply(DatabaseCommand::DeleteRow { row }).unwrap();
    assert_eq!(db.rows(), Vec::new());
    assert_eq!(db.column_ids(), Vec::new());
    assert!(db.undo().unwrap());
    assert!(db.undo().unwrap());
    assert_eq!(db.rows(), [row]);
    assert_eq!(names(&db), ["Name"]);
    assert_eq!(
        db.cell(row, name).unwrap(),
        Some(super::CellValue::Text("Lamp".into()))
    );
    assert!(db.redo().unwrap());
    assert_eq!(db.column_ids(), Vec::new());
    checked(&db);
}

#[test]
fn a_broken_database_is_reported_by_the_invariant_check_and_unreadable_ids_refuse_to_load() {
    use loro::{LoroDoc, LoroList, LoroMap, LoroMovableList, TreeParentId};
    let raw = LoroDoc::new();
    let columns = raw.get_tree("columns");
    columns.enable_fractional_index(0);
    let top = columns.create(TreeParentId::Root).unwrap();
    columns
        .get_meta(top)
        .unwrap()
        .insert("id", ColumnId::new().to_string())
        .unwrap();
    let nested = columns.create(top).unwrap();
    columns
        .get_meta(nested)
        .unwrap()
        .insert("id", ColumnId::new().to_string())
        .unwrap();
    let schema = raw.get_map("schema");
    schema.insert("not-a-column", 1).unwrap();
    let untyped = schema
        .insert_container(&ColumnId::new().to_string(), LoroMap::new())
        .unwrap();
    let options = untyped
        .insert_container("options", LoroMovableList::new())
        .unwrap();
    let twice = leaf_core::OptionId::new().to_string();
    for _ in 0..2 {
        options
            .push_container(LoroMap::new())
            .unwrap()
            .insert("id", twice.as_str())
            .unwrap();
    }
    options.push_container(LoroMap::new()).unwrap();
    options.push_container(LoroList::new()).unwrap();
    raw.commit();
    let db = Database::from_bytes(&raw.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    let broken = db.check_invariants().unwrap_err();
    let mut numbers: Vec<u8> = broken.iter().map(|v| v.invariant).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [2, 3, 3, 3, 4, 4, 4], "{broken:?}");
    assert!(broken.iter().all(|v| !v.to_string().is_empty()));

    // A node without a readable ID can't be addressed, so the database doesn't load.
    for id in [None, Some(loro::LoroValue::from(7_i64)), Some("nope".into())] {
        let raw = LoroDoc::new();
        let rows = raw.get_tree("rows");
        let node = rows.create(TreeParentId::Root).unwrap();
        if let Some(id) = id {
            rows.get_meta(node).unwrap().insert("id", id).unwrap();
        }
        raw.commit();
        assert!(matches!(
            Database::from_bytes(&raw.export(loro::ExportMode::Snapshot).unwrap()),
            Err(ModelError::InvalidDatabaseNode { part: "row", .. })
        ));
    }
}
