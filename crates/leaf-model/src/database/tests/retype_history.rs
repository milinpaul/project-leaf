//! E09-S12: a multi-select cell whose column changes type, is cleared or given a plain value, then becomes a
//! multi-select again walks back and forth through every state with undo and redo. The cell's map is
//! mergeable, so it must never be taken out and put back with stale options inside.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId, PageId};

use super::{add_column, add_row, database};
use crate::database::{CellValue, Database, DatabaseCommand, DatabaseOutcome, PropertyType};

fn option(db: &mut Database, column: ColumnId) -> OptionId {
    match db.apply(DatabaseCommand::AddOption {
        column,
        name: "Urgent".into(),
        color: "red".into(),
    }) {
        Ok(DatabaseOutcome::OptionCreated(id)) => id,
        other => panic!("expected an option, got {other:?}"),
    }
}

/// What the cell and its column look like: the state undo and redo must give back exactly.
fn state(db: &Database, row: PageId, column: ColumnId) -> (String, usize, Option<CellValue>) {
    let col = db.column(column).unwrap();
    (
        col.kind.type_name().to_owned(),
        col.options.len(),
        db.cell(row, column).unwrap(),
    )
}

/// Applies each command, then undoes back to the start and redoes to the end, checking every state.
fn walk(db: &mut Database, row: PageId, column: ColumnId, commands: Vec<DatabaseCommand>) {
    let mut states = vec![state(db, row, column)];
    for command in commands {
        // A command that changes nothing leaves no undo step, as in the proptest.
        if db.apply(command).unwrap() != DatabaseOutcome::Unchanged {
            states.push(state(db, row, column));
        }
    }
    for expected in states.iter().rev().skip(1) {
        assert!(db.undo().unwrap());
        assert_eq!(&state(db, row, column), expected, "undo");
        assert_eq!(db.check_invariants(), Ok(()));
    }
    for expected in states.iter().skip(1) {
        assert!(db.redo().unwrap());
        assert_eq!(&state(db, row, column), expected, "redo");
    }
}

fn set(row: PageId, column: ColumnId, value: Option<CellValue>) -> DatabaseCommand {
    DatabaseCommand::SetCell { row, column, value }
}

fn retype(column: ColumnId, kind: PropertyType) -> DatabaseCommand {
    DatabaseCommand::SetColumnType { column, kind }
}

#[test]
fn clearing_a_retyped_multi_select_cell_undoes_and_redoes_exactly() {
    // The proptest's shrunk case: clear while a select, delete the option, back to multi-select, set none.
    let mut db = database();
    let column = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let row = add_row(&mut db);
    let urgent = option(&mut db, column);
    let commands = vec![
        set(row, column, Some(CellValue::Options(BTreeSet::from([urgent])))),
        retype(column, PropertyType::Select),
        set(row, column, None),
        DatabaseCommand::DeleteOption {
            column,
            option: urgent,
        },
        retype(column, PropertyType::MultiSelect),
        set(row, column, Some(CellValue::Options(BTreeSet::new()))),
    ];
    walk(&mut db, row, column, commands);
}

#[test]
fn text_over_a_multi_select_cell_stays_in_its_map_and_undoes_and_redoes_exactly() {
    let mut db = database();
    let column = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let row = add_row(&mut db);
    let urgent = option(&mut db, column);
    let commands = vec![
        set(row, column, Some(CellValue::Options(BTreeSet::from([urgent])))),
        retype(column, PropertyType::Text),
        set(row, column, Some(CellValue::Text("call back".into()))),
        retype(column, PropertyType::MultiSelect),
        set(row, column, Some(CellValue::Options(BTreeSet::from([urgent])))),
        retype(column, PropertyType::Text),
    ];
    walk(&mut db, row, column, commands);

    // Once the cell holds options again, the old text is gone: back as text, the cell reads its options
    // rather than "call back".
    assert_ne!(
        db.cell(row, column).unwrap(),
        Some(CellValue::Text("call back".into()))
    );
}

#[test]
fn a_cleared_retyped_cell_reads_empty_and_text_reads_back() {
    let mut db = database();
    let column = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let row = add_row(&mut db);
    let urgent = option(&mut db, column);
    for command in [
        set(row, column, Some(CellValue::Options(BTreeSet::from([urgent])))),
        retype(column, PropertyType::Text),
        set(row, column, Some(CellValue::Text("call back".into()))),
    ] {
        db.apply(command).unwrap();
    }
    assert_eq!(
        db.cell(row, column).unwrap(),
        Some(CellValue::Text("call back".into()))
    );
    db.apply(set(row, column, None)).unwrap();
    assert_eq!(db.cell(row, column).unwrap(), None, "cleared");
    assert_eq!(
        db.apply(set(row, column, None)).unwrap(),
        DatabaseOutcome::Unchanged,
        "clearing again changes nothing"
    );
    db.apply(retype(column, PropertyType::MultiSelect)).unwrap();
    assert_eq!(
        db.cell(row, column).unwrap(),
        Some(CellValue::Options(BTreeSet::new())),
        "as a multi-select, the emptied cell has no tags"
    );
}
