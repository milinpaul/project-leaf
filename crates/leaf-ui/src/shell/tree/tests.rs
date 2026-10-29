//! The sidebar page tree in a real window: chevrons, inline rename, trash, drag-and-drop, and the
//! expanded pages surviving a reopen.

use std::path::{Path, PathBuf};

use gpui::{Entity, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px};
use leaf_core::PageId;
use leaf_store::ViewState;

use crate::shell::{Shell, Storage};

fn open_shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    redraw(cx);
    (shell, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A page titled `title` inside `parent`, made the way the sidebar does.
fn add(shell: &Entity<Shell>, parent: Option<PageId>, title: &str, cx: &mut VisualTestContext) -> PageId {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| match parent {
            Some(parent) => shell.new_page_inside(parent, window, cx),
            None => shell.new_page(window, cx),
        });
    });
    cx.simulate_input(title);
    redraw(cx);
    shell.read_with(cx, |shell, _| shell.editor().selected().unwrap())
}

/// The sidebar as indented labels.
fn outline(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .sidebar()
            .unwrap()
            .into_iter()
            .map(|row| format!("{}{}", "  ".repeat(row.depth), row.label))
            .collect()
    })
}

fn centre(cx: &mut VisualTestContext, selector: &'static str) -> Point<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center()
}

#[gpui::test]
fn the_chevron_collapses_and_expands_a_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let a = add(&shell, None, "A", cx);
    add(&shell, Some(a), "Inside", cx);
    assert_eq!(outline(&shell, cx), ["A", "  Inside"]);

    let chevron = centre(cx, "chevron-0");
    cx.simulate_click(chevron, Modifiers::none());
    redraw(cx);
    assert_eq!(outline(&shell, cx), ["A"]);
    let selected = shell.read_with(cx, |shell, _| shell.editor().selected());
    assert_ne!(selected, Some(a), "the chevron doesn't open the page");
    cx.simulate_click(chevron, Modifiers::none());
    redraw(cx);
    assert_eq!(outline(&shell, cx), ["A", "  Inside"]);
}

#[gpui::test]
fn clicking_a_row_opens_its_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let first = add(&shell, None, "First", cx);
    add(&shell, None, "Second", cx);
    let row = centre(cx, "page-row-0");
    cx.simulate_click(row, Modifiers::none());
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor().selected()),
        Some(first)
    );
}

#[gpui::test]
fn renaming_inline_keeps_the_title_on_enter_and_the_old_one_on_escape(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let page = add(&shell, None, "Draft", cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.start_rename(page, window, cx)));
    redraw(cx);
    cx.simulate_input("Plan"); // replaces the selected title
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert_eq!(outline(&shell, cx), ["Plan"]);
    let heading = shell.read_with(cx, |shell, cx| shell.title.read(cx).value().to_string());
    assert_eq!(heading, "Plan", "the open page's heading follows");

    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.start_rename(page, window, cx)));
    redraw(cx);
    cx.simulate_input("Oops");
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert_eq!(outline(&shell, cx), ["Plan"]);
    assert!(shell.read_with(cx, |shell, _| shell.tree.renaming.is_none()));
}

#[gpui::test]
fn moving_the_open_page_to_the_trash_closes_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    add(&shell, None, "Keep", cx);
    let bin = add(&shell, None, "Bin", cx);
    add(&shell, Some(bin), "Inside", cx);
    cx.update(|_, cx| shell.update(cx, |shell, cx| shell.trash_page(bin, cx)));
    redraw(cx);
    assert_eq!(outline(&shell, cx), ["Keep"]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.editor().selected()), None);
}

/// Presses on `from`, drags to `to` in small steps (as a hand would), and lets go.
fn drag(cx: &mut VisualTestContext, from: Point<gpui::Pixels>, to: Point<gpui::Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=8 {
        let t = step as f32 / 8.0;
        let at = point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
        redraw(cx);
    }
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    redraw(cx);
}

#[gpui::test]
fn dragging_a_row_onto_the_middle_of_another_nests_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    add(&shell, None, "A", cx);
    add(&shell, None, "B", cx);
    add(&shell, None, "C", cx);
    let from = centre(cx, "page-row-2");
    let to = centre(cx, "page-row-0");
    drag(cx, from, to);
    assert_eq!(outline(&shell, cx), ["A", "  C", "B"]);
}

#[gpui::test]
fn dragging_to_the_top_edge_of_a_row_puts_the_page_above_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    add(&shell, None, "A", cx);
    add(&shell, None, "B", cx);
    add(&shell, None, "C", cx);
    let from = centre(cx, "page-row-2");
    let top = cx.debug_bounds("page-row-0").unwrap();
    drag(cx, from, point(top.center().x, top.top() + px(2.0)));
    assert_eq!(outline(&shell, cx), ["C", "A", "B"]);
}

#[gpui::test]
fn a_page_dropped_onto_its_own_child_stays_put(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let a = add(&shell, None, "A", cx);
    add(&shell, Some(a), "Inside", cx);
    let from = centre(cx, "page-row-0");
    let to = centre(cx, "page-row-1");
    drag(cx, from, to);
    assert_eq!(outline(&shell, cx), ["A", "  Inside"]);
    assert!(shell.read_with(cx, |shell, _| shell.tree.drop_hint.is_none()));
}

#[gpui::test]
fn dropping_below_the_rows_moves_a_page_to_the_end_of_the_top_level(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    let a = add(&shell, None, "A", cx);
    add(&shell, Some(a), "Inside", cx);
    add(&shell, None, "B", cx);
    let from = centre(cx, "page-row-1");
    let area = cx.debug_bounds("page-tree-area").unwrap();
    drag(cx, from, point(area.center().x, area.bottom() - px(10.0)));
    assert_eq!(outline(&shell, cx), ["A", "B", "Inside"]);
}

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-ui-tree-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("w.leaf")
}

fn open_folder<'a>(path: &Path, cx: &'a mut TestAppContext) -> (Entity<Shell>, &'a mut VisualTestContext) {
    let folder = leaf_store::Workspace::open_or_create(path).unwrap();
    let (storage, workspace) = Storage::open(folder).unwrap();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell::with_storage(storage, workspace, window, cx));
    redraw(cx);
    (shell, cx)
}

#[gpui::test]
fn expanded_pages_are_remembered_in_the_workspace_folder(cx: &mut TestAppContext) {
    cx.update(|cx| crate::init(None, cx));
    let path = temp_workspace("expanded");
    let (a, b) = {
        let (shell, cx) = open_folder(&path, cx);
        let a = add(&shell, None, "A", cx);
        add(&shell, Some(a), "A1", cx);
        let b = add(&shell, None, "B", cx);
        add(&shell, Some(b), "B1", cx);
        // Collapse B; A stays open.
        cx.update(|_, cx| shell.update(cx, |shell, cx| shell.toggle_expanded(b, cx)));
        drop(shell); // closing the window then drops the shell, which releases the folder
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
        (a, b)
    };
    assert_eq!(
        ViewState::load(&path.join(leaf_store::VIEW_STATE)).expanded(),
        vec![a]
    );

    let (shell, cx) = open_folder(&path, cx);
    assert_eq!(outline(&shell, cx), ["A", "  A1", "B"]);
    assert!(!shell.read_with(cx, |shell, _| shell.editor().is_expanded(b)));
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}
