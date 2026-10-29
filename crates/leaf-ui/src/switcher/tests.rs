//! The quick switcher in a real window: Cmd/Ctrl+P opens it, typing narrows the list, the arrows
//! move the highlight and Enter opens the page.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use gpui_component::{Root, WindowExt as _};
use leaf_core::PageId;

use crate::menus::QuickSwitch;
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

fn selected(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.editor().selected())
}

#[gpui::test]
fn typing_and_enter_open_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let projects = add(&shell, None, "Projects", cx);
    let design = add(&shell, Some(projects), "Design notes", cx);
    add(&shell, None, "Journal", cx);

    cx.dispatch_action(QuickSwitch);
    redraw(cx);
    assert!(cx.update(Window::has_active_dialog));
    cx.simulate_input("desn");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(
        !cx.update(Window::has_active_dialog),
        "choosing closes the switcher"
    );
    assert_eq!(selected(&shell, cx), Some(design));
}

#[gpui::test]
fn arrows_move_the_highlight_and_escape_closes(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let first = add(&shell, None, "Alpha", cx);
    let second = add(&shell, None, "Beta", cx);
    // Recent pages lead with nothing typed: Beta (opened last), then Alpha.
    cx.dispatch_action(QuickSwitch);
    redraw(cx);
    cx.simulate_keystrokes("down enter");
    redraw(cx);
    assert_eq!(selected(&shell, cx), Some(first));

    cx.dispatch_action(QuickSwitch);
    redraw(cx);
    cx.simulate_keystrokes("up"); // wraps to the bottom row
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!cx.update(Window::has_active_dialog));
    assert_eq!(selected(&shell, cx), Some(first), "escape opens nothing");
    let _ = second;
}

#[gpui::test]
fn clicking_a_result_opens_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let alpha = add(&shell, None, "Alpha", cx);
    add(&shell, None, "Beta", cx);
    cx.dispatch_action(QuickSwitch);
    redraw(cx);
    cx.simulate_input("alp");
    redraw(cx);
    let row = cx.debug_bounds("switcher-row-0").unwrap().center();
    cx.simulate_click(row, Modifiers::none());
    redraw(cx);
    assert_eq!(selected(&shell, cx), Some(alpha));
    assert!(!cx.update(Window::has_active_dialog));
}

#[gpui::test]
fn no_match_shows_an_empty_state_and_enter_does_nothing(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let only = add(&shell, None, "Alpha", cx);
    cx.dispatch_action(QuickSwitch);
    redraw(cx);
    cx.simulate_input("zzz");
    redraw(cx);
    assert!(cx.debug_bounds("switcher-row-0").is_none());
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(
        cx.update(Window::has_active_dialog),
        "still open, nothing to open"
    );
    assert_eq!(selected(&shell, cx), Some(only));
}
