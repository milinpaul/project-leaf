//! The page-link menu's popover in a (test) window (E10-S05): typing `[[` shows it, typing filters it,
//! the keyboard and the mouse pick a page, Escape leaves the text.

use gpui::{Modifiers, MouseButton, MouseDownEvent, TestAppContext, VisualTestContext};
use leaf_core::{BlockId, PageId};
use leaf_model::Caret;

use crate::shell::Shell;

const NOW: i64 = 1_700_000_000_000;

/// "Trip to Japan" and "Japanese phrases" exist; a new page is open with the keyboard in its first line.
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId, [PageId; 2]) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    let pages = shell.update(cx, |shell, _| {
        let trip = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(trip, "Trip to Japan").unwrap();
        let phrases = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(phrases, "Japanese phrases").unwrap();
        [trip, phrases]
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        shell.editor.open_page().unwrap().unwrap().ensure_block().unwrap()
    });
    draw(cx);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block, at: 0 }, window, cx);
        });
    });
    draw(cx);
    (shell, cx, block, pages)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn links(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(BlockId, PageId)> {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().page_links())
}

fn text(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> String {
    shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().plain_text(block).unwrap().unwrap()
    })
}

#[gpui::test]
fn typing_brackets_shows_the_popover_and_the_keyboard_picks_a_page(cx: &mut TestAppContext) {
    let (shell, cx, block, [trip, _]) = open(cx);
    cx.simulate_input("see [");
    draw(cx);
    assert!(cx.debug_bounds("link-menu").is_none(), "one bracket is a bracket");
    cx.simulate_input("[");
    draw(cx);
    let menu = cx.debug_bounds("link-menu").expect("open under the brackets");
    let line = shell.read_with(cx, |shell, _| {
        shell.blocks.painted.caret_bounds(block, 4).unwrap()
    });
    assert!(menu.top() >= line.0.y + line.1, "below the line");

    cx.simulate_input("jap");
    draw(cx);
    assert!(cx.debug_bounds("link-item-2").is_some(), "two pages and New page");
    cx.simulate_keystrokes("down enter");
    draw(cx);
    assert!(cx.debug_bounds("link-menu").is_none());
    assert_eq!(
        links(&shell, cx),
        vec![(block, trip)],
        "the second match: after Japanese phrases"
    );
    // The keyboard stays in the page, right after the link.
    cx.simulate_input("!");
    draw(cx);
    assert_eq!(text(&shell, cx, block), format!("see {}!", leaf_model::PAGE_LINK));
}

#[gpui::test]
fn escape_leaves_the_typed_text(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    cx.simulate_input("[[jap");
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("link-menu").is_none());
    assert_eq!(text(&shell, cx, block), "[[jap");
    assert_eq!(links(&shell, cx), Vec::new());
}

#[gpui::test]
fn a_click_on_new_page_makes_and_links_it(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    cx.simulate_input("[[Kyoto");
    draw(cx);
    let new = cx
        .debug_bounds("link-item-0")
        .expect("New page is the only entry")
        .center();
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: new,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    let made = links(&shell, cx);
    assert_eq!(made.len(), 1);
    assert_eq!(made[0].0, block);
    let title = shell.update(cx, |shell, _| shell.editor.link_target(made[0].1).title);
    assert_eq!(title, "Kyoto");
}

/// "trip" typed and selected in the open page's first line, the floating bar drawn.
fn select_trip(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) {
    cx.simulate_input("see trip now");
    shell.update(cx, |shell, _| shell.page().unwrap().select(block, 4..8).unwrap());
    draw(cx);
    draw(cx);
}

#[gpui::test]
fn the_floating_bar_links_selected_words_to_a_page(cx: &mut TestAppContext) {
    let (shell, cx, block, [trip, _]) = open(cx);
    select_trip(&shell, cx, block);
    let button = cx
        .debug_bounds("format-link-to-page")
        .expect("Link to page in the floating bar")
        .center();
    cx.simulate_click(button, Modifiers::none());
    draw(cx);
    assert!(
        cx.debug_bounds("link-menu").is_some(),
        "the page list, searching for the words"
    );
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(links(&shell, cx), vec![(block, trip)]);
    assert_eq!(
        text(&shell, cx, block),
        format!("see {} now", leaf_model::PAGE_LINK)
    );
}

#[gpui::test]
fn escape_from_the_floating_bar_list_gives_the_words_back(cx: &mut TestAppContext) {
    let (shell, cx, block, _) = open(cx);
    select_trip(&shell, cx, block);
    let button = cx.debug_bounds("format-link-to-page").unwrap().center();
    cx.simulate_click(button, Modifiers::none());
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("link-menu").is_none());
    assert_eq!(text(&shell, cx, block), "see trip now");
}
