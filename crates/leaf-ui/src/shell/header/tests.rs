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

/// The emoji a search shows, in order, as the editor's picker has them.
fn results(query: &str) -> Vec<&'static str> {
    let mut picker = leaf_editor::EmojiPicker::new();
    picker.set_query(query);
    picker.sections()[0]
        .emojis
        .iter()
        .map(|choice| choice.emoji)
        .collect()
}

#[gpui::test]
fn arrow_keys_move_through_the_results_and_enter_picks(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-icon");
    cx.simulate_input("heart");
    redraw(cx);
    let found = results("heart");
    assert!(found.len() > 12, "enough hearts for two rows: {}", found.len());
    // Right twice, down a row, left once: the 12th result (row 2, column 2 of 10).
    cx.simulate_keystrokes("right right down left");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert_eq!(icon(&shell, cx).as_deref(), Some(found[11]));
    assert!(!picker_open(&shell, cx));

    // Typing still works after the arrows, and a new search starts from its best match again.
    click(cx, "page-icon");
    cx.simulate_keystrokes("down");
    cx.simulate_input("rocket");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert_eq!(icon(&shell, cx).as_deref(), Some("🚀"));
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
    click(cx, "cover-sage");
    assert_eq!(cover(&shell, cx), Some(PageCover::Builtin(BuiltinCover::Sage)));
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
    click(cx, "cover-sage");

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
    click(cx, "cover-sage");

    click(cx, "change-cover");
    assert!(cover_picker_open(&shell, cx));
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!cover_picker_open(&shell, cx), "Esc closes the picker");
    assert!(cover(&shell, cx).is_some(), "and changes nothing");
}

/// A cover's thumbnail selector in the picker. Selectors must be `'static`; leaking a few short strings
/// in a test is fine.
fn thumb(cover: leaf_editor::BuiltinCover) -> &'static str {
    format!("cover-{}", cover.name()).leak()
}

#[gpui::test]
fn the_picker_offers_every_built_in_cover_four_to_a_row(cx: &mut TestAppContext) {
    use leaf_editor::{BuiltinCover, CoverSection, PageCover};

    let (shell, cx) = open_window(cx);
    click(cx, "add-cover");
    for cover in BuiltinCover::ALL {
        let selector = thumb(*cover);
        assert!(cx.debug_bounds(selector).is_some(), "{selector} is offered");
    }
    let tops: Vec<f32> = CoverSection::ColourAndGradient
        .covers()
        .take(5)
        .map(|cover| f32::from(cx.debug_bounds(thumb(cover)).unwrap().top()))
        .collect();
    assert!(
        tops[..4].iter().all(|top| (top - tops[0]).abs() < 0.5),
        "the first four share a row: {tops:?}"
    );
    assert!(tops[4] > tops[0], "the fifth starts the next row: {tops:?}");
    let first_picture = cx.debug_bounds("cover-dawn").unwrap();
    let last_colour = cx.debug_bounds("cover-haze").unwrap();
    assert!(
        first_picture.top() > last_colour.bottom(),
        "illustrations come after colours and gradients"
    );
    click(cx, "cover-twilight");
    assert_eq!(
        cover(&shell, cx),
        Some(PageCover::Builtin(BuiltinCover::Twilight))
    );
}

#[gpui::test]
fn remove_in_the_picker_shows_only_with_a_cover_and_takes_it_away(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    click(cx, "add-cover");
    assert!(
        cx.debug_bounds("cover-picker-remove").is_none(),
        "nothing to remove yet"
    );
    click(cx, "cover-lagoon");

    click(cx, "change-cover");
    click(cx, "cover-picker-remove");
    assert_eq!(cover(&shell, cx), None);
    assert!(!cover_picker_open(&shell, cx), "removing closes the picker");
    assert!(cx.debug_bounds("page-cover").is_none());
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
