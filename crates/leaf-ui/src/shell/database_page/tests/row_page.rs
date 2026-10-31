//! A database row opened as a page, in a (test) window (E09-S08): the "Open" button, the properties under the
//! title, and editing them.

use gpui::{Entity, TestAppContext, VisualTestContext};
use leaf_core::{ColumnId, DatabaseId, PageId};
use leaf_model::{CellValue, PropertyType};

use super::{add_column, add_row, click, draw, new_database, open_window};
use crate::shell::Shell;

/// A database with "Number", "Checkbox" and "Select" columns after the starter "Tags", and one row "Fig" with
/// Number 2, opened as a page with the table's "Open" button. Returns the database, the row, and the columns.
fn open_fig(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
) -> (PageId, DatabaseId, PageId, [ColumnId; 3]) {
    let (page, id) = new_database(shell, cx);
    let number = add_column(shell, id, PropertyType::Number, cx);
    let check = add_column(shell, id, PropertyType::Checkbox, cx);
    let select = add_column(shell, id, PropertyType::Select, cx);
    let row = add_row(shell, page, cx);
    shell.update(cx, |shell, cx| {
        shell.editor_mut().rename(row, "Fig").unwrap();
        let editor = shell.editor_mut().database_editor(id).unwrap();
        editor.commit_text(row, number, "2").unwrap();
        shell.refresh_database_table(cx);
    });
    draw(cx);
    click(cx, "database-open-0");
    draw(cx);
    (page, id, row, [number, check, select])
}

fn value(
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

#[gpui::test]
fn open_shows_the_row_as_a_page_with_its_properties_under_the_title(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (database, _, row, _) = open_fig(&shell, cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor().selected()),
        Some(row)
    );
    let props = cx
        .debug_bounds("row-properties")
        .expect("the properties are drawn");
    for ix in 0..4 {
        assert!(
            cx.debug_bounds(Box::leak(format!("row-property-{ix}").into_boxed_str()))
                .is_some(),
            "property {ix}"
        );
    }
    assert!(props.top() > 0.0.into());
    let crumbs = shell.read_with(cx, |shell, _| shell.editor().breadcrumb(row).unwrap());
    assert!(
        crumbs
            .iter()
            .any(|c| matches!(c, leaf_editor::Crumb::Page { page, .. } if *page == database)),
        "the breadcrumb shows the database"
    );
}

#[gpui::test]
fn a_number_typed_on_the_page_is_in_the_table_and_enter_adds_no_block(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id, row, [number, ..]) = open_fig(&shell, cx);
    let blocks = shell.read_with(cx, |shell, _| shell.page_block_count());
    click(cx, "row-property-1");
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("12");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(value(&shell, id, row, number, cx), Some(CellValue::Number(12.0)));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.page_block_count()),
        blocks,
        "Enter stayed in the property"
    );
    let shown = shell.read_with(cx, |shell, cx| {
        let (_, table) = shell.database_parts().unwrap();
        table.read(cx).delegate().view.rows[0].cells[1].clone()
    });
    assert_eq!(
        shown,
        leaf_editor::CellDisplay::Number("12".into()),
        "the table shows it"
    );
}

#[gpui::test]
fn escape_drops_what_was_typed(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id, row, [number, ..]) = open_fig(&shell, cx);
    click(cx, "row-property-1");
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("99");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(value(&shell, id, row, number, cx), Some(CellValue::Number(2.0)));
}

#[gpui::test]
fn a_checkbox_ticks_and_a_select_takes_a_new_option(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, id, row, [_, check, select]) = open_fig(&shell, cx);
    click(cx, "row-property-2");
    assert_eq!(value(&shell, id, row, check, cx), Some(CellValue::Checkbox(true)));

    click(cx, "row-property-3");
    assert!(
        cx.debug_bounds("database-picker").is_some(),
        "the option picker opens"
    );
    cx.simulate_input("Ripe");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert!(matches!(
        value(&shell, id, row, select, cx),
        Some(CellValue::Option(_))
    ));
    assert!(
        cx.debug_bounds("database-picker").is_none(),
        "a select's picker closes"
    );
}
