//! Tables drawn and edited in a real window over a real workspace folder (E16-S03).

use std::path::{Path, PathBuf};

use gpui::{Modifiers, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px, size};
use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, PageOutcome, Workspace, WorkspaceCommand};
use leaf_store::DocKey;

use super::super::{Shell, Storage};

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-table-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("w.leaf")
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Saves a workspace at `path` with one page: a paragraph, then a `rows` × `columns` table with a
/// header row whose cells read "r0c0", "r0c1", ..., then a paragraph. Returns the page and table.
fn save_page(path: &Path, rows: usize, columns: usize) -> (PageId, BlockId) {
    let mut ws = Workspace::new();
    let Ok(Outcome::PageCreated(page)) = ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "Plans".into(),
        created_at: 1,
    }) else {
        panic!("expected a page");
    };
    let mut doc = PageDoc::new();
    let block = |doc: &mut PageDoc, command| match doc.apply(command) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("expected a block, got {other:?}"),
    };
    let paragraph = |index, text: &str| PageCommand::InsertBlock {
        parent: None,
        index,
        kind: BlockKind::Paragraph,
        text: text.into(),
    };
    block(&mut doc, paragraph(0, "before"));
    let table = block(
        &mut doc,
        PageCommand::InsertTable {
            parent: None,
            index: 1,
            rows,
            columns,
            header_row: true,
        },
    );
    block(&mut doc, paragraph(2, "after"));
    for (r, row) in doc.table_cells(table).unwrap().into_iter().enumerate() {
        for (c, cell) in row.into_iter().enumerate() {
            doc.apply(PageCommand::InsertText {
                id: cell,
                pos: 0,
                text: format!("r{r}c{c}"),
            })
            .unwrap();
        }
    }
    let mut folder = leaf_store::Workspace::open_or_create(path).unwrap();
    folder.store_mut().save(DocKey::Workspace, &ws, 1).unwrap();
    folder.store_mut().save(DocKey::Page(page), &doc, 1).unwrap();
    (page, table)
}

/// A window on the saved page, drawn.
fn open<'a>(
    path: &Path,
    page: PageId,
    cx: &'a mut TestAppContext,
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let folder = leaf_store::Workspace::open_or_create(path).unwrap();
    let (storage, workspace) = Storage::open(folder).unwrap();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::with_storage(storage, workspace, window, cx));
    cx.simulate_resize(size(px(1200.0), px(900.0)));
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(page, window, cx)));
    draw(cx);
    draw(cx);
    (shell, cx)
}

fn selector(name: &str, id: BlockId) -> &'static str {
    Box::leak(format!("{name}-{id}").into_boxed_str())
}

fn grid(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, table: BlockId) -> Vec<Vec<String>> {
    shell.update(cx, |shell, _| {
        let doc = shell.page().unwrap().doc();
        doc.table_cells(table)
            .unwrap()
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|c| doc.plain_text(c).unwrap().unwrap())
                    .collect()
            })
            .collect()
    })
}

fn cells(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, table: BlockId) -> Vec<Vec<BlockId>> {
    shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().table_cells(table).unwrap()
    })
}

#[gpui::test]
fn every_cell_is_drawn_in_a_grid(cx: &mut TestAppContext) {
    let path = temp_workspace("grid");
    let (page, table) = save_page(&path, 3, 2);
    let (shell, cx) = open(&path, page, cx);
    assert!(cx.debug_bounds(selector("table", table)).is_some());
    let cells = cells(&shell, cx, table);
    let bounds: Vec<Vec<gpui::Bounds<gpui::Pixels>>> = cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|&cell| cx.debug_bounds(selector("table-cell", cell)).unwrap())
                .collect()
        })
        .collect();
    for row in &bounds {
        assert!(row[0].right() <= row[1].left() + px(1.0), "columns side by side");
        assert_eq!(row[0].top(), row[1].top(), "a row's cells line up");
        assert_eq!(row[0].size.width, px(super::CELL_WIDTH));
    }
    assert!(
        bounds[0][0].bottom() <= bounds[1][0].top() + px(1.0),
        "rows stacked"
    );
    assert_eq!(bounds[1][0].left(), bounds[0][0].left(), "columns line up");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_click_puts_the_caret_in_a_cell_and_typing_goes_there(cx: &mut TestAppContext) {
    let path = temp_workspace("type");
    let (page, table) = save_page(&path, 2, 2);
    let (shell, cx) = open(&path, page, cx);
    let cell = cells(&shell, cx, table)[1][1];
    let at = cx.debug_bounds(selector("table-cell", cell)).unwrap();
    cx.simulate_click(at.center(), Modifiers::none());
    let caret = shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap().unwrap());
    assert_eq!(caret.block, cell);
    cx.simulate_input("!");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("x");
    assert_eq!(
        grid(&shell, cx, table),
        [["r0c0", "r0c1"], ["r1c0", "r1c1!"], ["x", ""]]
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn the_plus_bars_add_a_row_and_a_column(cx: &mut TestAppContext) {
    let path = temp_workspace("plus");
    let (page, table) = save_page(&path, 2, 2);
    let (shell, cx) = open(&path, page, cx);
    let add_row = cx.debug_bounds(selector("table-add-row", table)).unwrap();
    cx.simulate_click(add_row.center(), Modifiers::none());
    draw(cx);
    assert_eq!(grid(&shell, cx, table).len(), 3);
    let caret = shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap().unwrap());
    assert_eq!(caret.block, cells(&shell, cx, table)[2][0], "into the new row");

    let add_column = cx.debug_bounds(selector("table-add-column", table)).unwrap();
    cx.simulate_click(add_column.center(), Modifiers::none());
    draw(cx);
    assert_eq!(grid(&shell, cx, table)[0], ["r0c0", "r0c1", ""]);
    let new = cells(&shell, cx, table)[0][2];
    assert!(
        cx.debug_bounds(selector("table-cell", new)).is_some(),
        "the new column is drawn"
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_wide_table_scrolls_sideways_inside_the_page(cx: &mut TestAppContext) {
    let path = temp_workspace("wide");
    let (page, table) = save_page(&path, 2, 9);
    let (shell, cx) = open(&path, page, cx);
    let frame = cx.debug_bounds(selector("table", table)).unwrap();
    let first = cells(&shell, cx, table)[0][0];
    let last = cells(&shell, cx, table)[0][8];
    let before = cx.debug_bounds(selector("table-cell", first)).unwrap();
    assert!(
        cx.debug_bounds(selector("table-cell", last)).unwrap().right() > frame.right(),
        "the grid is wider than its box"
    );
    assert!(frame.right() <= px(1200.0), "the box stays inside the window");

    // Scrolling down over the table leaves it in place.
    let wheel = |delta, shift| ScrollEvent {
        position: frame.center(),
        delta,
        modifiers: Modifiers {
            shift,
            ..Modifiers::none()
        },
        ..ScrollEvent::default()
    };
    cx.simulate_event(wheel(ScrollDelta::Pixels(point(px(0.0), px(-30.0))), false));
    draw(cx);
    let moved = cx.debug_bounds(selector("table-cell", first)).unwrap();
    assert_eq!(moved.left(), before.left(), "not sideways");

    // A sideways swipe and Shift+wheel do scroll it.
    cx.simulate_event(wheel(ScrollDelta::Pixels(point(px(-100.0), px(0.0))), false));
    draw(cx);
    let swiped = cx.debug_bounds(selector("table-cell", first)).unwrap();
    assert!(swiped.left() < before.left(), "{swiped:?} vs {before:?}");
    cx.simulate_event(wheel(ScrollDelta::Lines(point(0.0, -3.0)), true));
    draw(cx);
    let shifted = cx.debug_bounds(selector("table-cell", first)).unwrap();
    assert!(shifted.left() < swiped.left(), "Shift+wheel scrolls further");
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[gpui::test]
fn a_control_edit_changes_the_table_through_the_editor(cx: &mut TestAppContext) {
    let path = temp_workspace("menu");
    let (page, table) = save_page(&path, 2, 2);
    let (shell, cx) = open(&path, page, cx);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.table_edit(
                table,
                leaf_editor::TableEdit::MoveRow { from: 1, to: 0 },
                window,
                cx,
            );
            shell.table_edit(table, leaf_editor::TableEdit::DeleteColumn(0), window, cx);
        });
    });
    assert_eq!(grid(&shell, cx, table), [["r1c1"], ["r0c1"]]);
    // A refused edit reports and changes nothing.
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.table_edit(table, leaf_editor::TableEdit::DeleteColumn(0), window, cx);
        });
    });
    assert_eq!(grid(&shell, cx, table), [["r1c1"], ["r0c1"]]);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
