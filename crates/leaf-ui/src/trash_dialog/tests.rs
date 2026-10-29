//! The Trash in a real window: restoring from the dialog, deleting for good after confirming,
//! emptying the trash, and the banner over a trashed page.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use gpui_component::{Root, WindowExt as _};
use leaf_core::PageId;

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
    redraw(cx);
    let shell = made.borrow().clone().unwrap();
    (shell, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn add(shell: &Entity<Shell>, title: &str, cx: &mut VisualTestContext) -> PageId {
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input(title);
    redraw(cx);
    shell.read_with(cx, |shell, _| shell.editor().selected().unwrap())
}

fn trash(shell: &Entity<Shell>, page: PageId, cx: &mut VisualTestContext) {
    cx.update(|_, cx| shell.update(cx, |shell, cx| shell.trash_page(page, cx)));
    redraw(cx);
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
}

fn titles(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .sidebar()
            .unwrap()
            .into_iter()
            .map(|i| i.label)
            .collect()
    })
}

fn in_trash(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> usize {
    shell.read_with(cx, |shell, _| shell.editor().trash_items().unwrap().len())
}

/// The OK button of the confirmation on top: the alert dialog's primary button.
fn confirm(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    redraw(cx);
}

#[gpui::test]
fn the_sidebar_opens_the_trash_and_restore_puts_a_page_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let garden = add(&shell, "Garden", cx);
    add(&shell, "Notes", cx);
    trash(&shell, garden, cx);
    assert_eq!(titles(&shell, cx), ["Notes"]);

    click(cx, "open-trash");
    assert!(cx.update(Window::has_active_dialog));
    assert!(cx.debug_bounds("trash-row-0").is_some());
    click(cx, "restore-0");
    assert_eq!(titles(&shell, cx), ["Garden", "Notes"]);
    assert_eq!(in_trash(&shell, cx), 0);
    assert!(
        cx.debug_bounds("trash-row-0").is_none(),
        "the dialog's list refreshed"
    );
}

#[gpui::test]
fn delete_forever_asks_first_and_cancel_keeps_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let garden = add(&shell, "Garden", cx);
    trash(&shell, garden, cx);
    click(cx, "open-trash");

    click(cx, "delete-0");
    cx.simulate_keystrokes("escape"); // cancel the confirmation
    redraw(cx);
    assert_eq!(in_trash(&shell, cx), 1, "nothing deleted without confirming");

    click(cx, "delete-0");
    confirm(cx);
    assert_eq!(in_trash(&shell, cx), 0);
    assert!(!shell.read_with(cx, |shell, _| shell.editor().workspace().contains(garden)));
}

#[gpui::test]
fn empty_trash_deletes_everything_after_confirming(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    for title in ["One", "Two", "Three"] {
        let page = add(&shell, title, cx);
        trash(&shell, page, cx);
    }
    add(&shell, "Kept", cx);
    click(cx, "open-trash");
    click(cx, "empty-trash");
    confirm(cx);
    assert_eq!(in_trash(&shell, cx), 0);
    assert_eq!(titles(&shell, cx), ["Kept"]);
}

#[gpui::test]
fn a_trashed_page_opens_with_a_banner_that_restores_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let garden = add(&shell, "Garden", cx);
    trash(&shell, garden, cx);
    click(cx, "open-trash");
    click(cx, "trash-row-0");
    assert!(
        !cx.update(Window::has_active_dialog),
        "opening a page closes the dialog"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor().selected()),
        Some(garden)
    );
    assert!(cx.debug_bounds("banner-restore").is_some());

    click(cx, "banner-restore");
    assert!(
        cx.debug_bounds("banner-restore").is_none(),
        "no banner once restored"
    );
    assert_eq!(titles(&shell, cx), ["Garden"]);
}

#[gpui::test]
fn the_banner_can_delete_the_open_page_for_good(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let garden = add(&shell, "Garden", cx);
    trash(&shell, garden, cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(garden, window, cx)));
    redraw(cx);
    click(cx, "banner-delete");
    confirm(cx);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.editor().selected()),
        None,
        "nothing open now"
    );
    assert_eq!(in_trash(&shell, cx), 0);
}
