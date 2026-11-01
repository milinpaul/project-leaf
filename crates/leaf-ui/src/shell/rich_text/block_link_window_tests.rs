//! Links to a block in a (test) window (E10-S08): drawn as the page and the block's words, a click that
//! opens the page at the block and flashes it, and "Copy link to block" in the ⋮⋮ menu.

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext};
use leaf_core::{BlockId, PageId};
use leaf_editor::Fragment;
use leaf_model::Caret;

use crate::shell::Shell;

/// A page "Bookings" (🏨) with "Flights" and "Book the ryokan", and, open, a page whose first line is
/// "see " + a link to the second block + " now". Returns the line, the target page and its two blocks.
fn open(
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<Shell>,
    &mut VisualTestContext,
    BlockId,
    PageId,
    [BlockId; 2],
) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let (target, blocks) = shell.update(cx, |shell, _| {
        let target = shell.editor.selected().unwrap();
        shell.editor.rename(target, "Bookings").unwrap();
        shell.editor.set_icon(target, Some("🏨")).unwrap();
        let page = shell.editor.open_page().unwrap().unwrap();
        let first = page.ensure_block().unwrap();
        page.set_text(first, "Flights").unwrap();
        let second = page.enter(first).unwrap();
        page.set_text(second, "Book the ryokan").unwrap();
        (target, [first, second])
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let line = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let line = page.ensure_block().unwrap();
        page.set_text(line, "see  now").unwrap();
        page.set_caret(Caret { block: line, at: 4 }).unwrap();
        page.paste(&Fragment::block_link(target, blocks[1])).unwrap();
        line
    });
    draw(cx);
    (shell, cx, line, target, blocks)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn click(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    draw(cx);
}

/// The middle of the link (between page positions 4 and 5), at mid-line height.
fn middle_of_link(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    line: BlockId,
) -> gpui::Point<gpui::Pixels> {
    shell.read_with(cx, |shell, _| {
        let (start, height) = shell.blocks.painted.caret_bounds(line, 4).unwrap();
        let (end, _) = shell.blocks.painted.caret_bounds(line, 5).unwrap();
        gpui::point((start.x + end.x) / 2.0, start.y + height / 2.0)
    })
}

#[gpui::test]
fn a_block_link_shows_the_blocks_words_and_a_click_opens_the_page_at_the_block(cx: &mut TestAppContext) {
    let (shell, cx, line, target, blocks) = open(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert_eq!(
        painted.text(line).as_deref(),
        Some("see 🏨 Bookings › Book the ryokan now")
    );
    let at = middle_of_link(&shell, cx, line);
    click(cx, at);
    let (selected, flashed, caret) = shell.update(cx, |shell, _| {
        (
            shell.editor.selected(),
            shell.flashed_block(),
            shell.editor.open_page().unwrap().unwrap().caret().unwrap(),
        )
    });
    assert_eq!(selected, Some(target));
    assert_eq!(flashed, Some(blocks[1]));
    assert_eq!(
        caret,
        Some(Caret {
            block: blocks[1],
            at: 0
        })
    );
}

#[gpui::test]
fn a_link_to_a_deleted_block_says_so_and_opens_the_page_at_the_top(cx: &mut TestAppContext) {
    let (shell, cx, line, target, blocks) = open(cx);
    shell.update(cx, |shell, _| {
        let source = shell.editor.selected().unwrap();
        shell.editor.select(target).unwrap();
        let page = shell.editor.open_page().unwrap().unwrap();
        page.delete_blocks(&[blocks[1]]).unwrap();
        shell.editor.select(source).unwrap();
    });
    draw(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert_eq!(
        painted.text(line).as_deref(),
        Some("see 🏨 Bookings › deleted block now")
    );
    let at = middle_of_link(&shell, cx, line);
    click(cx, at);
    let (selected, flashed) = shell.update(cx, |shell, _| (shell.editor.selected(), shell.flashed_block()));
    assert_eq!(selected, Some(target));
    assert_eq!(flashed, None);
}

#[gpui::test]
fn copy_link_to_block_in_the_menu_puts_the_link_on_the_clipboard(cx: &mut TestAppContext) {
    let (shell, cx, _, target, blocks) = open(cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(target, window, cx)));
    draw(cx);
    let at = cx
        .debug_bounds(Box::leak(format!("block-handle-{}", blocks[1]).into_boxed_str()))
        .unwrap()
        .center();
    click(cx, at);
    let entry = cx.debug_bounds("block-menu-5").unwrap().center();
    click(cx, entry);
    let item = cx.read_from_clipboard().unwrap();
    assert_eq!(item.text().as_deref(), Some("[[Bookings › Book the ryokan]]"));
    let fragment = Fragment::from_json(item.metadata().unwrap()).unwrap();
    assert_eq!(fragment, Fragment::block_link(target, blocks[1]));
    assert!(
        shell.update(cx, |shell, _| shell.block_menu().is_none()),
        "the menu closes"
    );
}
