//! The column menus in a (test) window (E09-S06): each action from the header, and undo.

use gpui::TestAppContext;
use leaf_model::{CellValue, PropertyType};

use super::{add_column, add_row, cell, click, draw, new_database, open_window};

/// The shown column names, in order.
fn shown(shell: &gpui::Entity<crate::shell::Shell>, cx: &mut gpui::VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, cx| {
        let (_, table) = shell.database_parts().unwrap();
        table
            .read(cx)
            .delegate()
            .view
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect()
    })
}

fn menu_open(shell: &gpui::Entity<crate::shell::Shell>, cx: &mut gpui::VisualTestContext) -> bool {
    shell.read_with(cx, crate::shell::Shell::column_menu_open)
}

#[gpui::test]
fn a_header_opens_its_menu_and_enter_renames_the_column(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-head-1");
    assert!(
        cx.debug_bounds("database-column-menu").is_some(),
        "the Tags column's menu"
    );
    assert!(
        cx.debug_bounds("database-column-left").is_none(),
        "nothing shown to its left"
    );

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Labels");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(shown(&shell, cx), ["Labels"]);
}

#[gpui::test]
fn escape_closes_the_menu_and_keeps_a_typed_name(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-head-1");
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Kinds");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!menu_open(&shell, cx));
    assert_eq!(shown(&shell, cx), ["Kinds"]);
}

#[gpui::test]
fn a_retype_keeps_the_values_and_undo_takes_it_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let notes = add_column(&shell, id, PropertyType::Text, cx);
    let row = add_row(&shell, page, cx);
    shell.update(cx, |shell, _| {
        shell
            .editor_mut()
            .database_editor(id)
            .unwrap()
            .commit_text(row, notes, "42")
            .unwrap();
    });

    click(cx, "database-head-2");
    click(cx, "database-column-type-1"); // Number
    let kind = shell.read_with(cx, |shell, _| {
        shell.editor().database(id).unwrap().column(notes).unwrap().kind
    });
    assert_eq!(kind, PropertyType::Number);
    assert_eq!(
        cell(&shell, id, row, notes, cx),
        Some(CellValue::Text("42".into())),
        "stored value kept"
    );

    click(cx, "database-cell-0-0");
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    let kind = shell.read_with(cx, |shell, _| {
        shell.editor().database(id).unwrap().column(notes).unwrap().kind
    });
    assert_eq!(kind, PropertyType::Text, "one undo step");
}

#[gpui::test]
fn move_right_then_left_keeps_the_menu_on_the_moved_column(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id) = new_database(&shell, cx);
    let _ = add_column(&shell, id, PropertyType::Number, cx);
    click(cx, "database-head-1");
    click(cx, "database-column-right");
    assert_eq!(shown(&shell, cx), ["Number", "Tags"]);
    assert!(menu_open(&shell, cx), "still open, now on the second column");
    assert!(
        cx.debug_bounds("database-column-right").is_none(),
        "it is last now"
    );
    click(cx, "database-column-left");
    assert_eq!(shown(&shell, cx), ["Tags", "Number"]);
}

#[gpui::test]
fn a_hidden_column_is_shown_again_from_the_plus_menu(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id) = new_database(&shell, cx);
    let _ = add_column(&shell, id, PropertyType::Date, cx);
    click(cx, "database-head-1");
    click(cx, "database-column-hide");
    assert_eq!(shown(&shell, cx), ["Date"]);

    click(cx, "database-add-column");
    assert!(cx.debug_bounds("database-add-menu").is_some());
    click(cx, "database-show-0");
    assert_eq!(shown(&shell, cx), ["Tags", "Date"], "back in its place");
}

#[gpui::test]
fn the_plus_menu_adds_a_column_and_opens_it_to_be_named(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-add-column");
    click(cx, "database-add-type-5"); // Checkbox
    assert_eq!(shown(&shell, cx), ["Tags", "Checkbox"]);
    assert!(
        cx.debug_bounds("database-column-menu").is_some(),
        "its menu, to name it"
    );
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Done");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Tags", "Done"]);
}

#[gpui::test]
fn delete_removes_the_column_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-head-1");
    click(cx, "database-column-delete");
    assert_eq!(shown(&shell, cx), Vec::<String>::new());

    click(cx, "database-head-0");
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Tags"]);
    cx.simulate_keystrokes("secondary-shift-z");
    draw(cx);
    assert_eq!(
        shown(&shell, cx),
        Vec::<String>::new(),
        "and redo deletes it again"
    );
}

#[gpui::test]
fn a_click_outside_closes_the_menu(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-head-1");
    assert!(menu_open(&shell, cx));
    click(cx, "database-new-row");
    assert!(!menu_open(&shell, cx));
}

#[gpui::test]
fn dragged_headers_and_edges_are_saved(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id) = new_database(&shell, cx);
    let number = add_column(&shell, id, PropertyType::Number, cx);
    // What the table reports for a header dropped on another, then for an edge let go: Tags onto Number.
    shell.update(cx, |shell, cx| shell.move_dragged_column(1, 2, cx));
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Number", "Tags"]);
    // A drop onto the title or the "+" column changes nothing.
    shell.update(cx, |shell, cx| shell.move_dragged_column(1, 0, cx));
    draw(cx);
    assert_eq!(shown(&shell, cx), ["Number", "Tags"]);

    let widths = [gpui::px(260.0), gpui::px(96.0), gpui::px(180.0), gpui::px(40.0)];
    shell.update(cx, |shell, cx| shell.resize_columns(&widths, cx));
    let width = shell.read_with(cx, |shell, _| {
        shell.editor().database(id).unwrap().column(number).unwrap().width
    });
    assert_eq!(width, Some(96.0));
}
