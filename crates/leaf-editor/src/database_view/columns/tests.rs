//! The column menu: what it offers and what each choice writes, each one undo step.

use leaf_core::{ColumnId, PageId};
use leaf_model::{CellValue, Database, DatabaseCommand, PropertyType};

use super::{COLUMN_TYPES, HiddenColumn, type_label};
use crate::DatabaseEditor;

/// Columns "A", "B", "C", "D" (text) and one row.
fn editor() -> (DatabaseEditor, PageId, Vec<ColumnId>) {
    // No merge interval set here: the editor makes each command its own undo step itself.
    let mut e = DatabaseEditor::new(Database::new());
    let columns: Vec<ColumnId> = ["A", "B", "C", "D"]
        .iter()
        .map(|name| {
            let c = e.add_column(PropertyType::Text).unwrap();
            assert!(e.rename_column(c, name).unwrap());
            c
        })
        .collect();
    let row = PageId::new();
    e.apply(DatabaseCommand::AddRow { row, index: 0 }).unwrap();
    (e, row, columns)
}

fn names(e: &DatabaseEditor) -> Vec<String> {
    e.database()
        .columns()
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[test]
fn every_type_is_offered_once_with_a_label() {
    let labels: Vec<_> = COLUMN_TYPES.iter().map(type_label).collect();
    assert_eq!(
        labels,
        [
            "Text",
            "Number",
            "Select",
            "Multi-select",
            "Date",
            "Checkbox",
            "URL"
        ]
    );
    assert_eq!(type_label(&PropertyType::Unknown("rollup".into())), "Unknown");
}

#[test]
fn new_columns_are_named_after_their_type_without_clashing() {
    let mut e = DatabaseEditor::new(Database::new());
    let first = e.add_column(PropertyType::Number).unwrap();
    e.add_column(PropertyType::Number).unwrap();
    e.add_column(PropertyType::Date).unwrap();
    assert_eq!(names(&e), ["Number", "Number 2", "Date"]);
    assert_eq!(e.database().column(first).unwrap().kind, PropertyType::Number);
}

#[test]
fn rename_trims_and_blank_keeps_the_old_name() {
    let (mut e, _, c) = editor();
    assert!(e.rename_column(c[0], "  Plant  ").unwrap());
    assert!(!e.rename_column(c[0], "   ").unwrap());
    assert_eq!(names(&e)[0], "Plant");
}

#[test]
fn a_retype_keeps_the_stored_values_and_can_be_undone() {
    let (mut e, row, c) = editor();
    e.commit_text(row, c[0], "12").unwrap();
    e.set_column_type(c[0], PropertyType::Number).unwrap();
    assert_eq!(e.database().column(c[0]).unwrap().kind, PropertyType::Number);
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Text("12".into())),
        "kept as stored"
    );

    assert!(e.undo().unwrap());
    assert_eq!(e.database().column(c[0]).unwrap().kind, PropertyType::Text);
}

#[test]
fn the_menu_knows_where_a_column_can_move() {
    let (mut e, _, c) = editor();
    let first = e.column_menu(c[0]).unwrap();
    assert_eq!(
        (first.name.as_str(), first.can_move_left, first.can_move_right),
        ("A", false, true)
    );
    assert!(first.retypable);
    // With D hidden, C is the last shown column.
    e.set_column_hidden(c[3], true).unwrap();
    let third = e.column_menu(c[2]).unwrap();
    assert_eq!((third.can_move_left, third.can_move_right), (true, false));
}

#[test]
fn stepping_skips_hidden_columns_and_stops_at_the_ends() {
    let (mut e, _, c) = editor();
    e.set_column_hidden(c[1], true).unwrap();
    assert!(e.step_column(c[0], false).unwrap());
    assert_eq!(names(&e), ["B", "C", "A", "D"], "past C, the next shown column");
    assert!(e.step_column(c[0], true).unwrap());
    assert_eq!(names(&e), ["B", "A", "C", "D"], "back before C");
    assert!(e.step_column(c[3], false).is_ok_and(|moved| !moved), "D is last");
    let b = e.step_column(c[1], true).unwrap();
    assert!(!b, "nothing shown left of B");

    assert!(e.undo().unwrap());
    assert_eq!(names(&e), ["B", "C", "A", "D"], "each move is one undo step");
}

#[test]
fn a_dragged_header_lands_next_to_the_one_it_was_dropped_on() {
    let (mut e, _, c) = editor();
    e.set_column_hidden(c[2], true).unwrap();
    // Shown: A B D. Drag A onto D.
    assert!(e.move_shown_column(0, 2).unwrap());
    assert_eq!(names(&e), ["B", "C", "D", "A"]);
    // Drag A (shown 2) back to the front.
    assert!(e.move_shown_column(2, 0).unwrap());
    assert_eq!(names(&e), ["A", "B", "C", "D"]);
    assert!(!e.move_shown_column(1, 1).unwrap());
    assert!(!e.move_shown_column(0, 9).unwrap(), "no such position");
}

#[test]
fn hidden_columns_are_listed_and_can_be_shown_again() {
    let (mut e, _, c) = editor();
    e.set_column_hidden(c[1], true).unwrap();
    assert_eq!(
        e.hidden_columns().unwrap(),
        [HiddenColumn {
            column: c[1],
            name: "B".into(),
            kind: PropertyType::Text
        }]
    );
    e.set_column_hidden(c[1], false).unwrap();
    assert_eq!(e.hidden_columns().unwrap(), []);
    assert!(e.undo().unwrap());
    assert_eq!(e.hidden_columns().unwrap().len(), 1);
}

#[test]
fn widths_round_and_ignore_tiny_or_bad_changes() {
    let (mut e, _, c) = editor();
    assert!(
        !e.set_column_width(c[0], super::super::DEFAULT_COLUMN_WIDTH + 0.2)
            .unwrap()
    );
    assert!(!e.set_column_width(c[0], f64::NAN).unwrap());
    assert!(!e.set_column_width(c[0], -3.0).unwrap());
    assert!(e.set_column_width(c[0], 240.4).unwrap());
    assert_eq!(e.database().column(c[0]).unwrap().width, Some(240.0));
}

#[test]
fn a_deleted_column_comes_back_whole_with_undo() {
    let (mut e, row, c) = editor();
    e.commit_text(row, c[1], "kept").unwrap();
    e.delete_column(c[1]).unwrap();
    assert_eq!(names(&e), ["A", "C", "D"]);
    assert!(e.column_menu(c[1]).is_err());
    assert!(e.undo().unwrap());
    assert_eq!(names(&e), ["A", "B", "C", "D"]);
    assert_eq!(
        e.database().cell(row, c[1]).unwrap(),
        Some(CellValue::Text("kept".into()))
    );
}

#[test]
fn quick_column_actions_still_undo_one_at_a_time() {
    let (mut e, _, c) = editor();
    e.set_column_hidden(c[0], true).unwrap();
    e.rename_column(c[1], "Bee").unwrap();
    assert!(e.undo().unwrap());
    assert_eq!(names(&e)[1], "B");
    assert!(
        e.database().column(c[0]).unwrap().hidden,
        "the hide is a step of its own"
    );
}
