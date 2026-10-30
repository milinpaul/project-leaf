//! The page menu's "Full width" switch in a (test) window (E18-S06): the page's title and blocks
//! widen to the page area and narrow again, per page.

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};
use leaf_core::PageId;

use super::tests::{draw, open_window, pages};
use crate::shell::Shell;
use crate::theme::READING_WIDTH;

/// How wide the open page's header and first block were drawn.
fn widths(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (f32, f32) {
    let header = cx.debug_bounds("page-header").unwrap().size.width;
    let block = shell.update(cx, |shell, _| {
        let first = shell.page().unwrap().view().unwrap()[0].id;
        shell.blocks.drawn.bounds_of(first).unwrap().size.width
    });
    (f32::from(header), f32::from(block))
}

fn is_wide(shell: &Entity<Shell>, page: PageId, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.editor().is_full_width(page).unwrap())
}

fn open_menu(cx: &mut VisualTestContext) {
    let button = cx.debug_bounds("page-menu").unwrap().center();
    cx.simulate_click(button, Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn the_switch_widens_the_page_and_back(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (_, notes) = pages(&shell, cx);
    let (header, block) = widths(&shell, cx);
    assert!(
        header <= READING_WIDTH && block <= READING_WIDTH,
        "a page starts in the reading column: {header} and {block}"
    );

    // Down to the third entry (Favourites, Rename, Full width); Enter flips it.
    open_menu(cx);
    cx.simulate_keystrokes("down down down enter");
    draw(cx);
    assert!(is_wide(&shell, notes, cx));
    let (wide_header, wide_block) = widths(&shell, cx);
    assert!(
        wide_header > READING_WIDTH && wide_block > READING_WIDTH,
        "the title and blocks use the page area: {wide_header} and {wide_block}"
    );

    // A click on the switch itself flips it back.
    open_menu(cx);
    let row = cx.debug_bounds("page-menu-Full width").unwrap();
    let on_switch = point(row.right() - px(16.0), row.center().y);
    cx.simulate_click(on_switch, Modifiers::none());
    draw(cx);
    assert!(!is_wide(&shell, notes, cx));
    assert_eq!(widths(&shell, cx), (header, block));
}

#[gpui::test]
fn each_page_opens_at_its_own_width(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (projects, notes) = pages(&shell, cx);
    shell.update(cx, |shell, _| {
        shell.editor_mut().set_full_width(notes, true).unwrap();
    });
    draw(cx);
    let wide = widths(&shell, cx);

    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(projects, window, cx)));
    draw(cx);
    let narrow = widths(&shell, cx);
    assert!(
        narrow.0 <= READING_WIDTH && narrow.1 <= READING_WIDTH,
        "{narrow:?}"
    );

    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(notes, window, cx)));
    draw(cx);
    assert_eq!(widths(&shell, cx), wide);
}
