//! A database page in a (test) window (E09-S05): the table is drawn, and each kind of cell edits.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use leaf_core::{ColumnId, DatabaseId, PageId};
use leaf_model::{CellValue, DatabaseCommand, DatabaseOutcome, PropertyType};

use super::EditKind;
use crate::shell::Shell;

fn open_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
    let keep = made.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let shell = cx.new(|cx| Shell::new(window, cx));
        *keep.borrow_mut() = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    draw(cx);
    let shell = made.borrow().clone().unwrap();
    (shell, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    draw(cx);
}

/// A new database page, open, via the sidebar's button.
fn new_database(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (PageId, DatabaseId) {
    click(cx, "add-database");
    shell.read_with(cx, |shell, _| shell.open_database().unwrap())
}

/// Adds a column of `kind` after the others.
fn add_column(
    shell: &Entity<Shell>,
    id: DatabaseId,
    kind: PropertyType,
    cx: &mut VisualTestContext,
) -> ColumnId {
    let column = shell.update(cx, |shell, cx| {
        let editor = shell.editor_mut().database_editor(id).unwrap();
        let index = editor.database().columns().unwrap().len();
        let DatabaseOutcome::ColumnCreated(column) = editor
            .apply(DatabaseCommand::AddColumn {
                index,
                name: format!("{kind:?}"),
                kind,
            })
            .unwrap()
        else {
            panic!("a column was added");
        };
        shell.refresh_database_table(cx);
        column
    });
    draw(cx);
    column
}

fn add_row(shell: &Entity<Shell>, page: PageId, cx: &mut VisualTestContext) -> PageId {
    let row = shell.update(cx, |shell, cx| {
        let row = shell.editor_mut().add_row(page, 0, 0).unwrap();
        shell.refresh_database_table(cx);
        row
    });
    draw(cx);
    row
}

fn begin_edit(shell: &Entity<Shell>, row: usize, col: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.begin_cell_edit(row, col, window, cx)));
    draw(cx);
}

fn cell(
    shell: &Entity<Shell>,
    id: DatabaseId,
    row: PageId,
    column: ColumnId,
    cx: &mut VisualTestContext,
) -> Option<CellValue> {
    shell.read_with(cx, |shell, _| {
        shell.editor().database(id).unwrap().cell(row, column).unwrap()
    })
}

fn editing(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<EditKind> {
    shell.read_with(cx, |shell, _| shell.editing().map(|e| e.kind))
}

#[gpui::test]
fn the_sidebar_button_opens_a_new_database_as_a_table(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    let name = cx.debug_bounds("database-head-0").expect("the title column");
    let tags = cx
        .debug_bounds("database-head-1")
        .expect("the starter Tags column");
    assert!(name.right() <= tags.left(), "Name comes first");
    assert!(cx.debug_bounds("database-cell-0-0").is_none(), "no rows yet");
    let new = cx.debug_bounds("database-new-row").expect("the New line");
    assert!(new.top() >= name.bottom(), "below the table's header");
}

#[gpui::test]
fn new_adds_a_row_and_its_title_is_typed_in_place(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let _ = new_database(&shell, cx);
    click(cx, "database-new-row");
    assert_eq!(
        editing(&shell, cx),
        Some(EditKind::Title),
        "typing the new row's title"
    );

    cx.simulate_input("Buy seeds");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(editing(&shell, cx), None);
    let title = shell.read_with(cx, |shell, cx| {
        shell.database_parts().unwrap().1.read(cx).delegate().view.rows[0]
            .title
            .clone()
    });
    assert_eq!(title, "Buy seeds");
    assert!(cx.debug_bounds("database-cell-0-0").is_some());
}

#[gpui::test]
fn text_commits_with_enter_and_escape_drops_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let notes = add_column(&shell, id, PropertyType::Text, cx);
    let row = add_row(&shell, page, cx);

    begin_edit(&shell, 0, 2, cx);
    assert_eq!(editing(&shell, cx), Some(EditKind::Text));
    cx.simulate_input("Water daily");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(
        cell(&shell, id, row, notes, cx),
        Some(CellValue::Text("Water daily".into()))
    );

    begin_edit(&shell, 0, 2, cx);
    cx.simulate_input(" and weed");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(editing(&shell, cx), None);
    assert_eq!(
        cell(&shell, id, row, notes, cx),
        Some(CellValue::Text("Water daily".into())),
        "Escape keeps what was there"
    );
}

#[gpui::test]
fn a_number_cell_stays_open_until_it_reads_as_a_number(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let count = add_column(&shell, id, PropertyType::Number, cx);
    let row = add_row(&shell, page, cx);

    begin_edit(&shell, 0, 2, cx);
    cx.simulate_input("lots");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(editing(&shell, cx), Some(EditKind::Text), "still open to fix");
    assert_eq!(cell(&shell, id, row, count, cx), None);

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("12");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(editing(&shell, cx), None);
    assert_eq!(cell(&shell, id, row, count, cx), Some(CellValue::Number(12.0)));
}

#[gpui::test]
fn a_checkbox_ticks_on_click(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let done = add_column(&shell, id, PropertyType::Checkbox, cx);
    let row = add_row(&shell, page, cx);

    click(cx, "database-check-0-2");
    assert_eq!(cell(&shell, id, row, done, cx), Some(CellValue::Checkbox(true)));
    click(cx, "database-check-0-2");
    assert_eq!(cell(&shell, id, row, done, cx), Some(CellValue::Checkbox(false)));
}

#[gpui::test]
fn typing_a_new_option_creates_it_and_the_picker_stays_open(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let row = add_row(&shell, page, cx);
    let tags = shell.read_with(cx, |shell, _| {
        shell.editor().database(id).unwrap().columns().unwrap()[0].id
    });

    begin_edit(&shell, 0, 1, cx);
    assert!(
        cx.debug_bounds("database-picker").is_some(),
        "the option picker opens"
    );
    cx.simulate_input("Urgent");
    draw(cx);
    assert!(
        cx.debug_bounds("database-create-option").is_some(),
        "offers to create it"
    );
    cx.simulate_keystrokes("enter");
    draw(cx);

    let Some(CellValue::Options(picked)) = cell(&shell, id, row, tags, cx) else {
        panic!("the cell has options");
    };
    assert_eq!(picked.len(), 1);
    assert_eq!(
        editing(&shell, cx),
        Some(EditKind::Options),
        "a multi-select picker stays open"
    );
    assert!(
        cx.debug_bounds("database-option-0").is_some(),
        "the new option is listed"
    );

    click(cx, "database-option-0");
    assert_eq!(
        cell(&shell, id, row, tags, cx),
        Some(CellValue::Options(std::collections::BTreeSet::new())),
        "picking it again takes it away"
    );
}

#[gpui::test]
fn a_select_cell_closes_its_picker_once_an_option_is_picked(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let status = add_column(&shell, id, PropertyType::Select, cx);
    let row = add_row(&shell, page, cx);

    begin_edit(&shell, 0, 2, cx);
    cx.simulate_input("Doing");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(matches!(
        cell(&shell, id, row, status, cx),
        Some(CellValue::Option(_))
    ));
    assert_eq!(editing(&shell, cx), None, "closed");
}

#[gpui::test]
fn a_date_cell_opens_a_calendar(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (page, id) = new_database(&shell, cx);
    let _ = add_column(&shell, id, PropertyType::Date, cx);
    let _ = add_row(&shell, page, cx);

    begin_edit(&shell, 0, 2, cx);
    assert_eq!(editing(&shell, cx), Some(EditKind::Date));
    assert!(cx.debug_bounds("database-calendar").is_some());
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(editing(&shell, cx), None);
}

mod columns;

mod query;

mod row_page;
