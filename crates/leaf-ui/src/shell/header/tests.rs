//! The page header in a real window: adding, changing and removing the icon and the cover.

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

fn cover(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<leaf_editor::PageCover> {
    shell.read_with(cx, |shell, _| shell.editor().selected_cover().unwrap())
}

fn cover_picker_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.cover_picker.is_some())
}

#[gpui::test]
fn add_cover_opens_the_picker_and_a_thumbnail_sets_the_cover(cx: &mut TestAppContext) {
    use leaf_editor::{BuiltinCover, PageCover};

    let (shell, cx) = open_window(cx);
    assert_eq!(cover(&shell, cx), None);
    assert!(cx.debug_bounds("page-cover").is_none(), "no band without a cover");
    click(cx, "add-cover");
    assert!(cover_picker_open(&shell, cx));
    click(cx, "cover-dawn");
    assert_eq!(cover(&shell, cx), Some(PageCover::Builtin(BuiltinCover::Dawn)));
    assert!(!cover_picker_open(&shell, cx), "picking closes the picker");

    let band = cx.debug_bounds("page-cover").expect("the cover band is drawn");
    let header = cx.debug_bounds("add-icon").expect("Add icon is still offered");
    assert!(
        band.size.height >= gpui::px(160.0) && band.size.height <= gpui::px(320.0),
        "the band's height stays within its limits: {band:?}"
    );
    assert!(header.top() >= band.bottom(), "the header sits under the band");
    assert!(
        cx.debug_bounds("add-cover").is_none(),
        "no Add cover once there is one"
    );
}

#[gpui::test]
fn the_icon_overlaps_the_cover_and_remove_takes_the_cover_away(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-icon");
    click(cx, "emoji-😀");
    click(cx, "add-cover");
    click(cx, "cover-dawn");

    let band = cx.debug_bounds("page-cover").unwrap();
    let icon = cx.debug_bounds("page-icon").unwrap();
    assert!(
        icon.top() < band.bottom() && icon.bottom() > band.bottom(),
        "the icon straddles the band's bottom edge: icon {icon:?}, band {band:?}"
    );

    click(cx, "remove-cover");
    assert_eq!(cover(&shell, cx), None);
    assert!(cx.debug_bounds("page-cover").is_none());
}

#[gpui::test]
fn change_cover_reopens_the_picker_and_escape_closes_it(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-cover");
    click(cx, "cover-dawn");

    click(cx, "change-cover");
    assert!(cover_picker_open(&shell, cx));
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!cover_picker_open(&shell, cx), "Esc closes the picker");
    assert!(cover(&shell, cx).is_some(), "and changes nothing");
}

#[gpui::test]
fn the_picker_offers_every_built_in_cover(cx: &mut TestAppContext) {
    use leaf_editor::{BuiltinCover, PageCover};

    let (shell, cx) = open_window(cx);
    click(cx, "add-cover");
    for selector in [
        "cover-dawn",
        "cover-forest",
        "cover-ocean",
        "cover-paper",
        "cover-night",
        "cover-travel",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} is offered");
    }
    click(cx, "cover-night");
    assert_eq!(cover(&shell, cx), Some(PageCover::Builtin(BuiltinCover::Night)));
}

#[gpui::test]
fn the_page_starts_with_its_header_not_a_toolbar(cx: &mut TestAppContext) {
    let (_shell, cx) = open_window(cx);
    let header = cx.debug_bounds("page-header").expect("the header is drawn");
    // Only the page's top padding sits above the header; the old row of "Turn into" buttons is gone.
    let top = f32::from(header.top());
    eprintln!("page header top: {top}");
    assert!(top < 80.0, "the header starts at the top of the page: {top}");
}
