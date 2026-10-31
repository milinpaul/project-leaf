//! Two copies of a database merging: columns, rows and cells combine, and a delete survives a concurrent move.

use leaf_core::{ColumnId, PageId};

use super::{add_column, add_row, checked, database, names};
use crate::ModelError;
use crate::database::{CellValue, Database, DatabaseCommand, PropertyType};

#[test]
fn two_copies_merge_columns_rows_and_cells() {
    let mut a = database();
    let name = add_column(&mut a, "Name", PropertyType::Text);
    let row = add_row(&mut a);
    let mut b = Database::from_bytes(&a.to_bytes().unwrap()).unwrap();
    a.apply(DatabaseCommand::RenameColumn {
        column: name,
        name: "Title".into(),
    })
    .unwrap();
    let other = add_row(&mut b);
    b.apply(DatabaseCommand::SetCell {
        row,
        column: name,
        value: Some(CellValue::Text("Lamp".into())),
    })
    .unwrap();
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    for db in [&a, &b] {
        assert_eq!(names(db), ["Title"]);
        assert_eq!(db.rows(), [row, other]);
        assert_eq!(db.cell(row, name).unwrap(), Some(CellValue::Text("Lamp".into())));
        checked(db);
    }
}

/// Two copies of a database with columns a, b, c and rows 1, 2, 3; the first is peer 1, the second peer 2.
fn two_copies() -> (Database, Database, Vec<ColumnId>, Vec<PageId>) {
    let mut a = database();
    let columns = ["a", "b", "c"]
        .map(|n| add_column(&mut a, n, PropertyType::Text))
        .to_vec();
    let rows = (0..3).map(|_| add_row(&mut a)).collect();
    let b = Database::from_bytes(&a.to_bytes().unwrap()).unwrap();
    (a, b, columns, rows)
}

fn merge(a: &mut Database, b: &mut Database) {
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
}

#[test]
fn a_column_or_row_deleted_on_one_copy_stays_deleted_when_the_other_moves_it_in_either_order() {
    // A tree alone settles a delete against a concurrent move by Loro's order of the two edits, so in one of
    // these the node comes back; the column's schema and the row's mark keep them deleted anyway.
    for delete_first in [true, false] {
        let (mut a, mut b, columns, rows) = two_copies();
        let delete = |db: &mut Database| {
            db.apply(DatabaseCommand::DeleteColumn { column: columns[0] })
                .unwrap();
            db.apply(DatabaseCommand::DeleteRow { row: rows[0] }).unwrap();
        };
        let mv = |db: &mut Database| {
            db.apply(DatabaseCommand::MoveColumn {
                column: columns[0],
                index: 2,
            })
            .unwrap();
            db.apply(DatabaseCommand::MoveRow {
                row: rows[0],
                index: 2,
            })
            .unwrap();
        };
        if delete_first {
            delete(&mut a);
            mv(&mut b);
        } else {
            mv(&mut a);
            delete(&mut b);
        }
        merge(&mut a, &mut b);
        for db in [&a, &b] {
            assert_eq!(db.column_ids(), columns[1..], "delete_first={delete_first}");
            assert_eq!(db.rows(), rows[1..], "delete_first={delete_first}");
            checked(db);
        }
    }
}

#[test]
fn a_deleted_row_added_again_comes_back_with_its_cells_where_asked() {
    let (mut db, _, columns, rows) = two_copies();
    db.apply(DatabaseCommand::SetCell {
        row: rows[0],
        column: columns[0],
        value: Some(CellValue::Text("kept".into())),
    })
    .unwrap();
    db.apply(DatabaseCommand::DeleteRow { row: rows[0] }).unwrap();
    assert!(matches!(
        db.apply(DatabaseCommand::MoveRow {
            row: rows[0],
            index: 0
        }),
        Err(ModelError::RowNotFound(_))
    ));
    db.apply(DatabaseCommand::AddRow {
        row: rows[0],
        index: 2,
    })
    .unwrap();
    assert_eq!(db.rows(), [rows[1], rows[2], rows[0]]);
    assert_eq!(
        db.cell(rows[0], columns[0]).unwrap(),
        Some(CellValue::Text("kept".into()))
    );
    checked(&db);
}

#[test]
fn a_deleted_row_is_restored_where_it_was() {
    let mut db = database();
    let rows: Vec<PageId> = (0..3).map(|_| add_row(&mut db)).collect();
    db.apply(DatabaseCommand::DeleteRow { row: rows[1] }).unwrap();
    db.apply(DatabaseCommand::RestoreRow { row: rows[1] }).unwrap();
    assert_eq!(db.rows(), rows, "back in the middle, where it was");
    assert!(matches!(
        db.apply(DatabaseCommand::RestoreRow { row: rows[1] }),
        Err(ModelError::DuplicateRow(_))
    ));
    assert!(matches!(
        db.apply(DatabaseCommand::RestoreRow { row: PageId::new() }),
        Err(ModelError::RowNotFound(_))
    ));
    checked(&db);
}
