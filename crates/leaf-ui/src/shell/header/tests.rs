//! The page header in a real window: adding an icon from the picker, changing it, removing it.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;

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
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    cx.simulate_input("Garden");
    redraw(cx);
    (shell, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    redraw(cx);
}

fn icon(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<String> {
    shell.read_with(cx, |shell, _| shell.editor().selected_icon().unwrap())
}

fn picker_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.icon_picker.is_some())
}

#[gpui::test]
fn searching_and_pressing_enter_sets_the_icon(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    assert_eq!(icon(&shell, cx), None);
    click(cx, "add-icon");
    assert!(picker_open(&shell, cx));
    assert!(
        cx.debug_bounds("emoji-😀").is_some(),
        "the grid starts with smileys"
    );
    cx.simulate_input("rocket");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert_eq!(icon(&shell, cx).as_deref(), Some("🚀"));
    assert!(!picker_open(&shell, cx), "picking closes the picker");
    let sidebar = shell.read_with(cx, |shell, _| shell.editor().sidebar().unwrap()[0].icon.clone());
    assert_eq!(sidebar.as_deref(), Some("🚀"), "the sidebar shows it");
}

#[gpui::test]
fn clicking_an_emoji_sets_it_and_remove_takes_it_away(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-icon");
    click(cx, "emoji-😀");
    assert_eq!(icon(&shell, cx).as_deref(), Some("😀"));

    click(cx, "page-icon"); // the icon itself opens the picker to change it
    assert!(picker_open(&shell, cx));
    click(cx, "remove-icon");
    assert_eq!(icon(&shell, cx), None);
    assert!(!picker_open(&shell, cx));
}

#[gpui::test]
fn random_picks_some_icon(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-icon");
    click(cx, "random-icon");
    assert!(icon(&shell, cx).is_some_and(|emoji| !emoji.is_empty()));
}
