//! The page's top bar in a (test) window (E18-S03).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext};
use gpui_component::Root;
use leaf_core::PageId;

use crate::menus::ToggleSidebar;
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
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"))
        .center();
    cx.simulate_click(at, Modifiers::none());
    draw(cx);
}

/// Pages Projects › Design › Notes, the last one open.
fn nested(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> [PageId; 3] {
    let pages = shell.update(cx, |shell, _| {
        let editor = shell.editor_mut();
        let projects = editor.new_page(0).unwrap();
        let design = editor.new_page_in(Some(projects), 0).unwrap();
        let notes = editor.new_page_in(Some(design), 0).unwrap();
        editor.rename(projects, "Projects").unwrap();
        editor.rename(design, "Design").unwrap();
        editor.rename(notes, "Notes").unwrap();
        [projects, design, notes]
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(pages[2], window, cx)));
    draw(cx);
    pages
}

fn selected(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.editor().selected())
}

#[gpui::test]
fn no_bar_without_an_open_page(cx: &mut TestAppContext) {
    let (_shell, cx) = open_window(cx);
    assert!(cx.debug_bounds("top-bar").is_none());
}

#[gpui::test]
fn the_breadcrumb_shows_the_path_and_opens_an_ancestor(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [projects, design, notes] = nested(&shell, cx);
    let bar = cx.debug_bounds("top-bar").unwrap();
    for crumb in [
        "crumb-0",
        "crumb-1",
        "crumb-2",
        "edited",
        "favourite",
        "page-menu",
    ] {
        let bounds = cx
            .debug_bounds(crumb)
            .unwrap_or_else(|| panic!("{crumb} missing"));
        assert!(bar.contains(&bounds.center()), "{crumb} sits in the bar");
    }
    assert_eq!(selected(&shell, cx), Some(notes));
    click(cx, "crumb-1");
    assert_eq!(
        selected(&shell, cx),
        Some(design),
        "the middle crumb opens Design"
    );
    assert!(
        cx.debug_bounds("crumb-2").is_none(),
        "Design's own path is two steps"
    );
    click(cx, "crumb-0");
    assert_eq!(selected(&shell, cx), Some(projects));
}

#[gpui::test]
fn the_star_adds_and_removes_the_page_from_favourites(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [_, _, notes] = nested(&shell, cx);
    let favourite =
        |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.editor().is_favourite(notes));
    assert!(!favourite(cx));
    click(cx, "favourite");
    assert!(favourite(cx));
    click(cx, "favourite");
    assert!(!favourite(cx));
}

#[gpui::test]
fn with_the_sidebar_hidden_its_button_sits_at_the_bars_left_end(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    nested(&shell, cx);
    assert!(cx.debug_bounds("show-sidebar").is_none());
    cx.dispatch_action(ToggleSidebar);
    draw(cx);
    let bar = cx.debug_bounds("top-bar").unwrap();
    let open = cx.debug_bounds("show-sidebar").unwrap();
    let first = cx.debug_bounds("crumb-0").unwrap();
    assert!(bar.contains(&open.center()), "in the bar");
    assert!(open.right() <= first.left(), "before the breadcrumb");
    click(cx, "show-sidebar");
    assert!(
        cx.debug_bounds("collapse-sidebar").is_some(),
        "the sidebar is back"
    );
}

#[gpui::test]
fn a_narrow_window_keeps_the_bar_on_one_line(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let [_, _, notes] = nested(&shell, cx);
    shell.update(cx, |shell, _| {
        let long =
            "A very long page title that cannot possibly fit beside everything else in a narrow window";
        shell.editor_mut().rename(notes, long).unwrap();
    });
    cx.simulate_resize(gpui::size(gpui::px(560.0), gpui::px(600.0)));
    draw(cx);
    let bar = cx.debug_bounds("top-bar").unwrap();
    assert!(
        f32::from(bar.size.height) <= super::TOP_BAR_PX + 0.5,
        "one line: {bar:?}"
    );
    // The controls on the right stay whole and inside the bar; the title gives way.
    for control in ["edited", "favourite", "page-menu"] {
        let bounds = cx.debug_bounds(control).unwrap();
        assert!(
            bar.contains(&bounds.center()) && bounds.right() <= bar.right(),
            "{control}: {bounds:?}"
        );
    }
    let crumbs = cx.debug_bounds("crumbs").unwrap();
    let edited = cx.debug_bounds("edited").unwrap();
    assert!(
        crumbs.right() <= edited.left(),
        "the breadcrumb ends before the controls: {crumbs:?}"
    );
}

#[test]
fn the_local_offset_is_a_whole_number_of_minutes_within_a_day() {
    let offset = super::local_offset_minutes(1_700_000_000_000);
    assert!((-24 * 60..=24 * 60).contains(&offset));
}
