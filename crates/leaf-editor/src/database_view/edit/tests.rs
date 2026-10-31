//! Editing cells in place: each type's editor and what each kind of input writes.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, PageId};
use leaf_model::{CellValue, Database, DatabaseCommand, DatabaseOutcome, PropertyType};

use super::{CellEditor, OPTION_COLORS, TextCommit};
use crate::DatabaseEditor;

fn editor(kinds: &[PropertyType]) -> (DatabaseEditor, PageId, Vec<ColumnId>) {
    let mut db = Database::new();
    let columns = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| {
            match db.apply(DatabaseCommand::AddColumn {
                index,
                name: kind.type_name().to_owned(),
                kind: kind.clone(),
            }) {
                Ok(DatabaseOutcome::ColumnCreated(id)) => id,
                other => panic!("expected a column, got {other:?}"),
            }
        })
        .collect();
    let row = PageId::new();
    db.apply(DatabaseCommand::AddRow { row, index: 0 }).unwrap();
    (DatabaseEditor::new(db), row, columns)
}

#[test]
fn each_type_opens_its_own_editor_starting_from_the_cell() {
    let (mut e, row, c) = editor(&[
        PropertyType::Text,
        PropertyType::Number,
        PropertyType::Url,
        PropertyType::Checkbox,
        PropertyType::Select,
        PropertyType::MultiSelect,
        PropertyType::Date,
    ]);
    e.commit_text(row, c[0], "Lamp").unwrap();
    e.commit_text(row, c[1], "2.5").unwrap();
    e.set_date(row, c[6], Some(86_400_000)).unwrap();
    let editors: Vec<CellEditor> = c.iter().map(|&col| e.cell_editor(row, col).unwrap()).collect();
    assert_eq!(
        editors,
        [
            CellEditor::Text {
                initial: "Lamp".into(),
                number: false
            },
            CellEditor::Text {
                initial: "2.5".into(),
                number: true
            },
            CellEditor::Text {
                initial: String::new(),
                number: false
            },
            CellEditor::Checkbox,
            CellEditor::Options { multiple: false },
            CellEditor::Options { multiple: true },
            CellEditor::Date {
                initial: Some(86_400_000)
            },
        ]
    );
}

#[test]
fn typed_text_is_written_by_type_and_a_bad_number_keeps_the_editor_open() {
    let (mut e, row, c) = editor(&[PropertyType::Text, PropertyType::Number, PropertyType::Url]);
    assert_eq!(
        e.commit_text(row, c[0], " two words ").unwrap(),
        TextCommit::Saved
    );
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Text(" two words ".into())),
        "text is kept as typed"
    );
    assert_eq!(e.commit_text(row, c[1], "12,5").unwrap(), TextCommit::NotANumber);
    assert_eq!(e.database().cell(row, c[1]).unwrap(), None, "nothing written");
    assert_eq!(e.commit_text(row, c[1], " -3e2 ").unwrap(), TextCommit::Saved);
    assert_eq!(
        e.database().cell(row, c[1]).unwrap(),
        Some(CellValue::Number(-300.0))
    );
    e.commit_text(row, c[2], " https://example.com ").unwrap();
    assert_eq!(
        e.database().cell(row, c[2]).unwrap(),
        Some(CellValue::Text("https://example.com".into())),
        "an address is trimmed"
    );
    e.commit_text(row, c[0], "   ").unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        None,
        "blank empties the cell"
    );
}

#[test]
fn checkboxes_toggle() {
    let (mut e, row, c) = editor(&[PropertyType::Checkbox]);
    e.toggle_checkbox(row, c[0]).unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Checkbox(true))
    );
    e.toggle_checkbox(row, c[0]).unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Checkbox(false))
    );
}

#[test]
fn options_are_created_in_turn_coloured_picked_and_found_by_name() {
    let (mut e, row, c) = editor(&[PropertyType::Select, PropertyType::MultiSelect]);
    let todo = e.create_option(row, c[0], " To do ").unwrap();
    let done = e.create_option(row, c[0], "Done").unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Option(done)),
        "a select keeps one"
    );
    let column = e.database().column(c[0]).unwrap();
    let names: Vec<(&str, &str)> = column
        .options
        .iter()
        .map(|o| (o.name.as_str(), o.color.as_str()))
        .collect();
    assert_eq!(names, [("To do", OPTION_COLORS[0]), ("Done", OPTION_COLORS[1])]);

    let picker = e.option_picker(row, c[0], "").unwrap();
    let shown: Vec<(&str, bool)> = picker
        .choices
        .iter()
        .map(|o| (o.name.as_str(), o.picked))
        .collect();
    assert_eq!(shown, [("To do", false), ("Done", true)]);
    assert_eq!(picker.create, None);
    let picker = e.option_picker(row, c[0], "do").unwrap();
    assert_eq!(picker.choices.len(), 2);
    assert_eq!(picker.create.as_deref(), Some("do"));
    assert_eq!(
        e.option_picker(row, c[0], "done").unwrap().create,
        None,
        "an exact name isn't offered again"
    );
    assert_eq!(e.option_picker(row, c[0], "zzz").unwrap().choices, Vec::new());

    // Picking the picked option in a select lets it go; picking another replaces it.
    e.toggle_option(row, c[0], done).unwrap();
    assert_eq!(e.database().cell(row, c[0]).unwrap(), None);
    e.toggle_option(row, c[0], todo).unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Option(todo))
    );

    // A multi-select adds and removes.
    let red = e.create_option(row, c[1], "red").unwrap();
    let blue = e.create_option(row, c[1], "blue").unwrap();
    assert_eq!(
        e.database().cell(row, c[1]).unwrap(),
        Some(CellValue::Options(BTreeSet::from([red, blue])))
    );
    e.toggle_option(row, c[1], red).unwrap();
    assert_eq!(
        e.database().cell(row, c[1]).unwrap(),
        Some(CellValue::Options(BTreeSet::from([blue])))
    );
}

#[test]
fn dates_are_set_and_cleared_and_unknown_types_are_read_only() {
    let (mut e, row, c) = editor(&[PropertyType::Date, PropertyType::Text]);
    e.set_date(row, c[0], Some(5)).unwrap();
    assert_eq!(e.database().cell(row, c[0]).unwrap(), Some(CellValue::Date(5)));
    e.set_date(row, c[0], None).unwrap();
    assert_eq!(e.database().cell(row, c[0]).unwrap(), None);
    assert!(e.toggle_option(row, c[1], leaf_core::OptionId::new()).is_err());
}

#[test]
fn creating_an_option_and_picking_it_is_one_undo_step() {
    let (mut e, row, c) = editor(&[PropertyType::Select]);
    let option = e.create_option(row, c[0], "Doing").unwrap();
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Option(option))
    );
    assert!(e.undo().unwrap());
    assert_eq!(e.database().cell(row, c[0]).unwrap(), None);
    assert!(
        e.database().column(c[0]).unwrap().options.is_empty(),
        "the option went with its pick"
    );
    assert!(e.redo().unwrap());
    assert_eq!(
        e.database().cell(row, c[0]).unwrap(),
        Some(CellValue::Option(option))
    );
}
