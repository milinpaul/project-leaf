//! An option's menu in the picker, in a (test) window (E09-S11): the "⋯" opens it; the name, a colour and "Delete"
//! act on the option; Escape goes back to the list; options drag to a new place. On a row page too.

use std::collections::BTreeSet;

use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use leaf_core::{ColumnId, DatabaseId, OptionId, PageId};
use leaf_model::CellValue;

use super::{add_row, begin_edit, click, draw, new_database, open_window};
use crate::shell::Shell;

/// A new database with one row whose Tags (the starter column, table column 1) has options Home and Work, both
/// picked; its picker open.
struct Fixture {
    id: DatabaseId,
    row: PageId,
    tags: ColumnId,
    home: OptionId,
    work: OptionId,
}

fn fixture(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Fixture {
    let (page, id) = new_database(shell, cx);
    let row = add_row(shell, page, cx);
    let (tags, home, work) = shell.update(cx, |shell, cx| {
        let editor = shell.editor_mut().database_editor(id).unwrap();
        let tags = editor.database().columns().unwrap()[0].id;
        let home = editor.create_option(row, tags, "Home").unwrap();
        let work = editor.create_option(row, tags, "Work").unwrap();
        shell.refresh_database_table(cx);
        (tags, home, work)
    });
    draw(cx);
    begin_edit(shell, 0, 1, cx);
    Fixture {
        id,
        row,
        tags,
        home,
        work,
    }
}

fn names(shell: &Entity<Shell>, f: &Fixture, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .database(f.id)
            .unwrap()
            .column(f.tags)
            .unwrap()
            .options
            .into_iter()
            .map(|o| o.name)
            .collect()
    })
}

fn cell(shell: &Entity<Shell>, f: &Fixture, cx: &mut VisualTestContext) -> Option<CellValue> {
    shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .database(f.id)
            .unwrap()
            .cell(f.row, f.tags)
            .unwrap()
    })
}

/// Hovers option `ix` of the picker, so its "⋯" shows, and clicks the "⋯".
fn open_menu(cx: &mut VisualTestContext, ix: usize) {
    let row: &'static str = Box::leak(format!("database-option-{ix}").into_boxed_str());
    let at = cx.debug_bounds(row).expect("the option is listed").center();
    cx.simulate_mouse_move(at, None, Modifiers::none());
    draw(cx);
    click(
        cx,
        Box::leak(format!("database-option-more-{ix}").into_boxed_str()),
    );
}

#[gpui::test]
fn the_menu_renames_an_option_and_escape_goes_back_to_the_list(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let f = fixture(&shell, cx);
    open_menu(cx, 0);
    assert!(
        cx.debug_bounds("database-option-menu").is_some(),
        "the menu shows"
    );
    assert!(
        cx.debug_bounds("database-option-0").is_none(),
        "in place of the list"
    );
    assert_eq!(
        cell(&shell, &f, cx),
        Some(CellValue::Options(BTreeSet::from([f.home, f.work]))),
        "nothing picked"
    );

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("House");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(names(&shell, &f, cx), ["House", "Work"]);
    assert!(
        cx.debug_bounds("database-option-menu").is_none(),
        "Enter goes back to the list"
    );
    assert!(cx.debug_bounds("database-picker").is_some());

    open_menu(cx, 1);
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("house");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(
        names(&shell, &f, cx),
        ["House", "Work"],
        "another option's name keeps the old one"
    );
    assert!(
        cx.debug_bounds("database-option-menu").is_none(),
        "Escape goes back to the list"
    );
    assert!(
        cx.debug_bounds("database-picker").is_some(),
        "the picker stays open"
    );
}

#[gpui::test]
fn a_colour_recolours_the_option_and_the_menu_stays(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let f = fixture(&shell, cx);
    open_menu(cx, 1);
    click(cx, "database-option-colour-4");
    let color = shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .database(f.id)
            .unwrap()
            .column(f.tags)
            .unwrap()
            .options[1]
            .color
            .clone()
    });
    assert_eq!(color, "green");
    assert!(cx.debug_bounds("database-option-menu").is_some());
}

#[gpui::test]
fn delete_takes_the_option_out_of_the_list_and_the_cell_and_undo_brings_both_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let f = fixture(&shell, cx);
    open_menu(cx, 0);
    click(cx, "database-option-delete");
    assert_eq!(names(&shell, &f, cx), ["Work"]);
    assert_eq!(
        cell(&shell, &f, cx),
        Some(CellValue::Options(BTreeSet::from([f.work])))
    );
    assert!(cx.debug_bounds("database-picker").is_some(), "back to the list");
    assert!(cx.debug_bounds("database-option-1").is_none(), "one option left");

    cx.simulate_keystrokes("escape");
    draw(cx);
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    assert_eq!(names(&shell, &f, cx), ["Home", "Work"]);
    assert_eq!(
        cell(&shell, &f, cx),
        Some(CellValue::Options(BTreeSet::from([f.home, f.work])))
    );
}

#[gpui::test]
fn an_option_dragged_onto_another_takes_its_place(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let f = fixture(&shell, cx);
    let from = cx.debug_bounds("database-option-1").unwrap().center();
    let to = cx.debug_bounds("database-option-0").unwrap().center();
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        from + gpui::point(gpui::px(0.0), gpui::px(-6.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(names(&shell, &f, cx), ["Work", "Home"]);
    assert_eq!(
        cell(&shell, &f, cx),
        Some(CellValue::Options(BTreeSet::from([f.home, f.work]))),
        "dropping picks nothing"
    );
}

#[gpui::test]
fn a_row_pages_picker_has_the_same_menu(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let f = fixture(&shell, cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    click(cx, "database-open-0");
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor().selected()),
        Some(f.row)
    );
    click(cx, "row-property-0");
    assert!(
        cx.debug_bounds("database-picker").is_some(),
        "the row page's picker"
    );
    open_menu(cx, 1);
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Office");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(names(&shell, &f, cx), ["Home", "Office"]);
    open_menu(cx, 0);
    click(cx, "database-option-delete");
    assert_eq!(names(&shell, &f, cx), ["Office"]);
    assert_eq!(
        cell(&shell, &f, cx),
        Some(CellValue::Options(BTreeSet::from([f.work])))
    );
}
