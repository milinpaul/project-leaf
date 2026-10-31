//! The web bookmark card in a (test) window (E15-S04).

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
use leaf_core::BlockId;
use leaf_model::Caret;

use crate::shell::Shell;

const LEAF: &str = "https://www.leaf.app/blog";

/// A page with a line of text and a web bookmark below it; returns the two blocks.
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let (text, bookmark) = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let text = page.ensure_block().unwrap();
        page.set_text(text, "Read later").unwrap();
        page.set_caret(Caret { block: text, at: 10 }).unwrap();
        page.insert_web_bookmark(LEAF).unwrap();
        let rows = page.rows().unwrap();
        (text, rows[1].id)
    });
    draw(cx);
    (shell, cx, text, bookmark)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn a_bookmark_is_drawn_as_a_card_with_its_address_before_a_preview(cx: &mut TestAppContext) {
    let (_shell, cx, _, bookmark) = open(cx);
    let card = cx
        .debug_bounds(Box::leak(format!("bookmark-{bookmark}").into_boxed_str()))
        .unwrap();
    assert!(card.size.height > gpui::px(40.0), "a card, not a line");
    assert!(
        cx.debug_bounds(Box::leak(format!("bookmark-title-{bookmark}").into_boxed_str()))
            .is_some()
    );
    assert!(
        cx.debug_bounds(Box::leak(
            format!("bookmark-fetching-{bookmark}").into_boxed_str()
        ))
        .is_none()
    );
}

#[gpui::test]
fn while_fetching_the_card_says_so(cx: &mut TestAppContext) {
    let (shell, cx, _, bookmark) = open(cx);
    shell.update(cx, |shell, cx| {
        shell.blocks.fetching.insert(bookmark);
        cx.notify();
    });
    draw(cx);
    assert!(
        cx.debug_bounds(Box::leak(
            format!("bookmark-fetching-{bookmark}").into_boxed_str()
        ))
        .is_some()
    );
}

#[gpui::test]
fn a_click_opens_the_address_and_leaves_the_caret(cx: &mut TestAppContext) {
    let (shell, cx, text, bookmark) = open(cx);
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .set_caret(Caret { block: text, at: 4 })
            .unwrap();
    });
    draw(cx);
    let at = cx
        .debug_bounds(Box::leak(format!("bookmark-{bookmark}").into_boxed_str()))
        .unwrap()
        .center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(cx.opened_url().as_deref(), Some(LEAF));
    let caret = shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap());
    assert_eq!(caret, Some(Caret { block: text, at: 4 }), "the caret stays");
}

#[gpui::test]
fn delete_above_selects_the_card_and_backspace_deletes_it_whole(cx: &mut TestAppContext) {
    let (shell, cx, text, bookmark) = open(cx);
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .set_caret(Caret { block: text, at: 10 })
            .unwrap();
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.focus_page(window, cx)));
    draw(cx);
    cx.simulate_keystrokes("delete");
    draw(cx);
    let selected = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert_eq!(selected, Some(leaf_editor::Selection::Blocks(vec![bookmark])));
    cx.simulate_keystrokes("backspace");
    draw(cx);
    let gone = shell.update(cx, |shell, _| !shell.page().unwrap().doc().contains(bookmark));
    assert!(gone, "deleted whole");
}
