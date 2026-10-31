//! Sorting and filtering from the bar above the table, in a (test) window (E09-S07).

use gpui::{Entity, TestAppContext, VisualTestContext};
use leaf_core::{ColumnId, DatabaseId};
use leaf_model::{CellValue, PropertyType};

use super::{add_column, add_row, click, draw, new_database, open_window};
use crate::shell::Shell;

/// A database with a "Number" column (shown as the second table column) and rows Pear 3, Apple 1, Fig 2.
fn fruit(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (DatabaseId, ColumnId) {
    let (page, id) = new_database(shell, cx);
    let number = add_column(shell, id, PropertyType::Number, cx);
    for (title, n) in [("Fig", 2.0), ("Apple", 1.0), ("Pear", 3.0)] {
        let row = add_row(shell, page, cx);
        shell.update(cx, |shell, cx| {
            shell.editor_mut().rename(row, title).unwrap();
            let editor = shell.editor_mut().database_editor(id).unwrap();
            editor.commit_text(row, number, &n.to_string()).unwrap();
            shell.refresh_database_table(cx);
        });
    }
    draw(cx);
    (id, number)
}

/// The titles the table shows, in order.
fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, cx| {
        let (_, table) = shell.database_parts().unwrap();
        table
            .read(cx)
            .delegate()
            .view
            .rows
            .iter()
            .map(|r| r.title.clone())
            .collect()
    })
}

fn menu_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.query_menu().is_some())
}

#[gpui::test]
fn sort_by_a_column_turn_it_round_and_remove_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = fruit(&shell, cx);
    assert_eq!(shown(&shell, cx), ["Pear", "Apple", "Fig"]);

    click(cx, "database-sort-button");
    assert!(
        cx.debug_bounds("database-sort-column-menu").is_some(),
        "columns to sort by"
    );
    click(cx, "database-sort-column-1"); // Number (Tags is first)
    assert_eq!(shown(&shell, cx), ["Apple", "Fig", "Pear"]);
    assert!(cx.debug_bounds("database-sort-chip-0").is_some());
    assert!(
        cx.debug_bounds("database-sorts-menu").is_some(),
        "the sorts, open"
    );

    click(cx, "database-sort-turn-0");
    assert_eq!(shown(&shell, cx), ["Pear", "Fig", "Apple"]);
    click(cx, "database-sort-remove-0");
    assert_eq!(shown(&shell, cx), ["Pear", "Apple", "Fig"]);
    assert!(!menu_open(&shell, cx), "closed with the last sort");
    assert!(cx.debug_bounds("database-sort-chip-0").is_none());
}

#[gpui::test]
fn a_typed_filter_shows_at_once_and_is_saved_on_enter(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (id, number) = fruit(&shell, cx);
    click(cx, "database-filter-button");
    click(cx, "database-filter-column-1"); // Number
    assert!(cx.debug_bounds("database-filter-menu").is_some());
    click(cx, "database-filter-op-2"); // ">"
    cx.simulate_input("1");
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Pear", "Fig"], "while typing");
    let saved = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            shell.editor().database(id).unwrap().filters()[0].value.clone()
        })
    };
    assert_eq!(saved(&shell, cx), None, "not written while typing");
    assert!(cx.debug_bounds("database-row-count").is_some(), "2 of 3");

    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(saved(&shell, cx), Some(CellValue::Number(1.0)));
    assert_eq!(shown(&shell, cx), ["Pear", "Fig"]);
    let _ = number;
}

#[gpui::test]
fn escape_closes_a_filter_and_keeps_what_was_typed(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = fruit(&shell, cx);
    click(cx, "database-filter-button");
    click(cx, "database-filter-column-1");
    cx.simulate_input("2");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(shown(&shell, cx), ["Fig"]);
}

#[gpui::test]
fn a_select_filter_picks_an_option_and_a_checkbox_filter_turns(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (id, _) = fruit(&shell, cx);
    let status = add_column(&shell, id, PropertyType::Select, cx);
    let done = add_column(&shell, id, PropertyType::Checkbox, cx);
    shell.update(cx, |shell, cx| {
        let rows = shell.editor().database(id).unwrap().rows();
        let editor = shell.editor_mut().database_editor(id).unwrap();
        editor.create_option(rows[2], status, "Ripe").unwrap(); // Fig (rows are Pear, Apple, Fig)
        editor.toggle_checkbox(rows[1], done).unwrap(); // Apple
        shell.refresh_database_table(cx);
    });
    draw(cx);

    click(cx, "database-filter-button");
    click(cx, "database-filter-column-2"); // Select
    click(cx, "database-filter-option-0");
    assert_eq!(shown(&shell, cx), ["Fig"]);
    click(cx, "database-filter-delete");
    assert_eq!(shown(&shell, cx), ["Pear", "Apple", "Fig"]);

    click(cx, "database-filter-button");
    click(cx, "database-filter-column-3"); // Checkbox: "is ticked" at once
    assert_eq!(shown(&shell, cx), ["Apple"]);
    click(cx, "database-filter-check-1");
    assert_eq!(shown(&shell, cx), ["Pear", "Fig"]);
}

#[gpui::test]
fn undo_takes_a_filter_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = fruit(&shell, cx);
    click(cx, "database-filter-button");
    click(cx, "database-filter-column-1");
    cx.simulate_input("3");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Pear"]);
    click(cx, "database-cell-0-0");
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    assert_eq!(
        shown(&shell, cx),
        ["Pear", "Apple", "Fig"],
        "the typed value went in one step"
    );
}
