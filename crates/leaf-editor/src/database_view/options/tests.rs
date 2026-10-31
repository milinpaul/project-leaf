//! Editing a column's options (E09-S11): the menu, renaming and its refusals, recolouring, moving, and deleting an
//! option from every cell as one undo step.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId, PageId};
use leaf_model::{CellValue, Database, DatabaseCommand, DatabaseOutcome, ModelError, PropertyType};

use crate::{DatabaseEditor, OPTION_COLORS};

/// A database with a Status (select) and a Tags (multi-select) column, two rows, and options picked in both.
struct Fixture {
    e: DatabaseEditor,
    rows: [PageId; 2],
    status: ColumnId,
    tags: ColumnId,
    /// Status options: Todo, Done.
    todo: OptionId,
    done: OptionId,
    /// Tags options: Home, Work.
    home: OptionId,
    work: OptionId,
}

fn fixture() -> Fixture {
    let mut db = Database::new();
    db.set_undo_merge_interval(0);
    let column = |db: &mut Database, index, kind| match db.apply(DatabaseCommand::AddColumn {
        index,
        name: "c".into(),
        kind,
    }) {
        Ok(DatabaseOutcome::ColumnCreated(id)) => id,
        other => panic!("expected a column, got {other:?}"),
    };
    let status = column(&mut db, 0, PropertyType::Select);
    let tags = column(&mut db, 1, PropertyType::MultiSelect);
    let rows = [PageId::new(), PageId::new()];
    for (index, row) in rows.iter().enumerate() {
        db.apply(DatabaseCommand::AddRow { row: *row, index }).unwrap();
    }
    let mut e = DatabaseEditor::new(db);
    let todo = e.create_option(rows[0], status, "Todo").unwrap();
    let done = e.create_option(rows[1], status, "Done").unwrap();
    let home = e.create_option(rows[0], tags, "Home").unwrap();
    let work = e.create_option(rows[0], tags, "Work").unwrap();
    e.toggle_option(rows[1], tags, home).unwrap();
    Fixture {
        e,
        rows,
        status,
        tags,
        todo,
        done,
        home,
        work,
    }
}

fn names(e: &DatabaseEditor, column: ColumnId) -> Vec<String> {
    e.database()
        .column(column)
        .unwrap()
        .options
        .into_iter()
        .map(|o| o.name)
        .collect()
}

#[test]
fn the_menu_shows_the_name_the_colour_and_the_palette() {
    let f = fixture();
    let menu = f.e.option_menu(f.status, f.done).unwrap();
    assert_eq!(menu.name, "Done");
    assert_eq!(
        menu.color, OPTION_COLORS[1],
        "the second option takes the second colour"
    );
    assert_eq!(menu.colors.len(), 9);
    assert_eq!(
        menu.colors
            .iter()
            .filter(|(_, on)| *on)
            .map(|(c, _)| *c)
            .collect::<Vec<_>>(),
        [OPTION_COLORS[1]]
    );
    assert!(matches!(
        f.e.option_menu(f.status, OptionId::new()),
        Err(ModelError::OptionNotFound { .. })
    ));
}

#[test]
fn renaming_trims_and_keeps_the_old_name_when_blank_the_same_or_taken() {
    let mut f = fixture();
    assert!(f.e.rename_option(f.status, f.done, "  Finished ").unwrap());
    assert_eq!(names(&f.e, f.status), ["Todo", "Finished"]);
    assert!(!f.e.rename_option(f.status, f.done, "   ").unwrap(), "blank");
    assert!(
        !f.e.rename_option(f.status, f.done, "Finished").unwrap(),
        "the same"
    );
    assert!(
        !f.e.rename_option(f.status, f.done, "todo").unwrap(),
        "another option's name, any case"
    );
    assert_eq!(names(&f.e, f.status), ["Todo", "Finished"]);
    assert!(
        f.e.rename_option(f.status, f.done, "FINISHED").unwrap(),
        "its own name in another case"
    );
}

#[test]
fn a_colour_comes_from_the_palette() {
    let mut f = fixture();
    assert!(f.e.set_option_color(f.status, f.todo, "green").unwrap());
    assert_eq!(f.e.option_menu(f.status, f.todo).unwrap().color, "green");
    assert!(
        !f.e.set_option_color(f.status, f.todo, "green").unwrap(),
        "already"
    );
    assert!(
        !f.e.set_option_color(f.status, f.todo, "teal").unwrap(),
        "not in the palette"
    );
    assert_eq!(f.e.option_menu(f.status, f.todo).unwrap().color, "green");
}

#[test]
fn an_option_moves_and_the_picker_lists_the_new_order() {
    let mut f = fixture();
    assert!(f.e.move_option(f.status, f.done, 0).unwrap());
    assert_eq!(names(&f.e, f.status), ["Done", "Todo"]);
    assert!(!f.e.move_option(f.status, f.done, 0).unwrap(), "already first");
    let picker = f.e.option_picker(f.rows[0], f.status, "").unwrap();
    let listed: Vec<&str> = picker.choices.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(listed, ["Done", "Todo"]);
    assert!(f.e.undo().unwrap());
    assert_eq!(names(&f.e, f.status), ["Todo", "Done"]);
}

#[test]
fn deleting_an_option_clears_it_from_every_cell_and_undo_brings_both_back() {
    let mut f = fixture();
    let cells = |e: &DatabaseEditor, column| -> Vec<Option<CellValue>> {
        [0, 1]
            .map(|i| e.database().cell(f.rows[i], column).unwrap())
            .to_vec()
    };
    let tags_before = cells(&f.e, f.tags);
    assert_eq!(
        tags_before,
        [
            Some(CellValue::Options(BTreeSet::from([f.home, f.work]))),
            Some(CellValue::Options(BTreeSet::from([f.home]))),
        ]
    );

    f.e.delete_option(f.tags, f.home).unwrap();
    assert_eq!(names(&f.e, f.tags), ["Work"]);
    assert_eq!(
        cells(&f.e, f.tags),
        [
            Some(CellValue::Options(BTreeSet::from([f.work]))),
            Some(CellValue::Options(BTreeSet::new()))
        ],
        "kept the other tag; a cell left with none has none"
    );
    f.e.delete_option(f.status, f.todo).unwrap();
    assert_eq!(cells(&f.e, f.status), [None, Some(CellValue::Option(f.done))]);

    assert!(f.e.undo().unwrap(), "one step for the status delete");
    assert_eq!(
        cells(&f.e, f.status),
        [Some(CellValue::Option(f.todo)), Some(CellValue::Option(f.done))]
    );
    assert!(f.e.undo().unwrap(), "one step for the tag delete, cells and all");
    assert_eq!(names(&f.e, f.tags), ["Home", "Work"]);
    assert_eq!(cells(&f.e, f.tags), tags_before);
    assert!(matches!(
        f.e.delete_option(f.tags, OptionId::new()),
        Err(ModelError::OptionNotFound { .. })
    ));
}
