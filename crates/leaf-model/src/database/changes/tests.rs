//! Changes reported for each kind of edit, and for undo and merges.

use std::collections::HashSet;

use leaf_core::PageId;

use crate::database::tests::{add_column, add_row, database};
use crate::database::{CellValue, Database, DatabaseChanges, DatabaseCommand, PropertyType};

#[test]
fn each_kind_of_edit_reports_what_it_touched_and_taking_forgets_it() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let tags = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let (a, b) = (add_row(&mut db), add_row(&mut db));
    let first = db.take_changes();
    assert!(first.columns && first.order);
    assert_eq!(db.take_changes(), DatabaseChanges::default(), "taken once");

    db.apply(DatabaseCommand::SetCell {
        row: a,
        column: name,
        value: Some(CellValue::Text("Lamp".into())),
    })
    .unwrap();
    let changes = db.take_changes();
    assert_eq!(changes.rows, HashSet::from([a]));
    assert!(!changes.order && !changes.columns && !changes.view);

    db.apply(DatabaseCommand::SetCell {
        row: b,
        column: tags,
        value: Some(CellValue::Options(std::collections::BTreeSet::new())),
    })
    .unwrap();
    db.apply(DatabaseCommand::SetCell {
        row: b,
        column: name,
        value: Some(CellValue::Text("Desk".into())),
    })
    .unwrap();
    assert_eq!(db.take_changes().rows, HashSet::from([b]));

    // A row edited again: its map already exists, and the change is still the row's.
    db.apply(DatabaseCommand::SetCell {
        row: a,
        column: name,
        value: Some(CellValue::Text("Lamp 2".into())),
    })
    .unwrap();
    db.apply(DatabaseCommand::SetCell {
        row: b,
        column: name,
        value: None,
    })
    .unwrap();
    assert_eq!(db.take_changes().rows, HashSet::from([a, b]));

    db.apply(DatabaseCommand::MoveRow { row: b, index: 0 }).unwrap();
    assert!(db.take_changes().order);
    db.apply(DatabaseCommand::DeleteRow { row: a }).unwrap();
    assert!(db.take_changes().order);
    db.apply(DatabaseCommand::RenameColumn {
        column: name,
        name: "Title".into(),
    })
    .unwrap();
    assert!(db.take_changes().columns);
    db.apply(DatabaseCommand::SetSort(vec![crate::Sort::new(name, true)]))
        .unwrap();
    assert!(db.take_changes().view);
}

#[test]
fn undo_and_merged_changes_are_reported_too() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let row = add_row(&mut db);
    db.apply(DatabaseCommand::SetCell {
        row,
        column: name,
        value: Some(CellValue::Text("one".into())),
    })
    .unwrap();
    db.take_changes();
    assert!(db.undo().unwrap());
    assert_eq!(db.take_changes().rows, HashSet::from([row]));

    let mut other = Database::from_bytes(&db.to_bytes().unwrap()).unwrap();
    assert_eq!(
        other.take_changes(),
        DatabaseChanges::default(),
        "loading isn't a change"
    );
    other
        .apply(DatabaseCommand::SetCell {
            row,
            column: name,
            value: Some(CellValue::Text("two".into())),
        })
        .unwrap();
    let new_row = PageId::new();
    other
        .apply(DatabaseCommand::AddRow {
            row: new_row,
            index: 1,
        })
        .unwrap();
    db.import_changes(&other.to_bytes().unwrap()).unwrap();
    let merged = db.take_changes();
    assert!(merged.rows.contains(&row) && merged.order, "{merged:?}");
}

#[test]
fn a_rows_values_are_read_in_one_go() {
    let mut db = database();
    let name = add_column(&mut db, "Name", PropertyType::Text);
    let price = add_column(&mut db, "Price", PropertyType::Number);
    let row = add_row(&mut db);
    assert_eq!(db.row_values(row).unwrap(), Vec::new());
    for (column, value) in [
        (name, CellValue::Text("Lamp".into())),
        (price, CellValue::Number(9.5)),
    ] {
        db.apply(DatabaseCommand::SetCell {
            row,
            column,
            value: Some(value),
        })
        .unwrap();
    }
    let mut values = db.row_values(row).unwrap();
    values.sort_by_key(|(column, _)| *column);
    assert_eq!(
        values,
        [
            (name, CellValue::Text("Lamp".into())),
            (price, CellValue::Number(9.5))
        ]
    );
    assert!(db.row_values(PageId::new()).is_err());
}
