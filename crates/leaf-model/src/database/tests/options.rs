//! Moving a column's options (E09-S11): to any place, refused out of range, undone in one step, and two copies moving
//! at once still read each option once.

use leaf_core::{ColumnId, OptionId};

use super::{add_column, checked, database};
use crate::ModelError;
use crate::database::{Database, DatabaseCommand, DatabaseOutcome, PropertyType};

fn add_option(db: &mut Database, column: ColumnId, name: &str) -> OptionId {
    match db.apply(DatabaseCommand::AddOption {
        column,
        name: name.into(),
        color: "gray".into(),
    }) {
        Ok(DatabaseOutcome::OptionCreated(id)) => id,
        other => panic!("expected an option, got {other:?}"),
    }
}

fn order(db: &Database, column: ColumnId) -> Vec<String> {
    db.column(column)
        .unwrap()
        .options
        .into_iter()
        .map(|o| o.name)
        .collect()
}

/// A status column with options Todo, Doing, Done.
fn status() -> (Database, ColumnId, [OptionId; 3]) {
    let mut db = database();
    let column = add_column(&mut db, "Status", PropertyType::Select);
    let ids = ["Todo", "Doing", "Done"].map(|name| add_option(&mut db, column, name));
    (db, column, ids)
}

fn move_to(
    db: &mut Database,
    column: ColumnId,
    option: OptionId,
    index: usize,
) -> Result<DatabaseOutcome, ModelError> {
    db.apply(DatabaseCommand::MoveOption {
        column,
        option,
        index,
    })
}

#[test]
fn an_option_moves_to_any_place_and_undo_puts_it_back() {
    let (mut db, column, [todo, doing, done]) = status();
    assert_eq!(move_to(&mut db, column, done, 0).unwrap(), DatabaseOutcome::Done);
    assert_eq!(order(&db, column), ["Done", "Todo", "Doing"]);
    move_to(&mut db, column, done, 2).unwrap();
    assert_eq!(order(&db, column), ["Todo", "Doing", "Done"]);
    move_to(&mut db, column, todo, 1).unwrap();
    assert_eq!(order(&db, column), ["Doing", "Todo", "Done"]);
    assert_eq!(
        move_to(&mut db, column, doing, 0).unwrap(),
        DatabaseOutcome::Unchanged,
        "already there"
    );
    checked(&db);

    assert!(db.undo().unwrap());
    assert_eq!(order(&db, column), ["Todo", "Doing", "Done"]);
    let reloaded = Database::from_bytes(&db.to_bytes().unwrap()).unwrap();
    assert_eq!(order(&reloaded, column), ["Todo", "Doing", "Done"]);
}

#[test]
fn a_move_out_of_range_or_of_a_missing_option_is_refused() {
    let (mut db, column, [todo, ..]) = status();
    assert!(matches!(
        move_to(&mut db, column, todo, 3),
        Err(ModelError::IndexOutOfRange { index: 3, len: 3 })
    ));
    assert!(matches!(
        move_to(&mut db, column, OptionId::new(), 0),
        Err(ModelError::OptionNotFound { .. })
    ));
    let text = add_column(&mut db, "Notes", PropertyType::Text);
    assert!(matches!(
        move_to(&mut db, text, todo, 0),
        Err(ModelError::NotASelectColumn(_))
    ));
    assert_eq!(order(&db, column), ["Todo", "Doing", "Done"]);
}

#[test]
fn two_copies_moving_options_at_once_agree_and_list_each_once() {
    let (mut a, column, [todo, _, done]) = status();
    let mut b = Database::from_bytes(&a.to_bytes().unwrap()).unwrap();
    move_to(&mut a, column, done, 0).unwrap();
    move_to(&mut b, column, todo, 2).unwrap();
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    assert_eq!(order(&a, column), order(&b, column));
    assert_eq!(order(&a, column).len(), 3);
    checked(&a);
}
