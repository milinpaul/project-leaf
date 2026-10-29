//! Dragging blocks in a (test) window (E07-S05): the drop line, dropping, auto-scroll.

use gpui::{Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px};
use leaf_core::BlockId;

use super::{scroll_speed, with_insides};
use crate::shell::Shell;

fn open<'a>(
    cx: &'a mut TestAppContext,
    texts: &[String],
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext, Vec<BlockId>) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let ids = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let mut ids = vec![page.ensure_block().unwrap()];
        page.set_text(ids[0], &texts[0]).unwrap();
        for text in &texts[1..] {
            let next = page.enter(*ids.last().unwrap()).unwrap();
            page.set_text(next, text).unwrap();
            ids.push(next);
        }
        ids
    });
    draw(cx);
    (shell, cx, ids)
}

fn words(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|t| (*t).to_owned()).collect()
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Each row's depth and text, top to bottom.
fn outline(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(usize, String)> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .rows()
            .unwrap()
            .into_iter()
            .map(|row| (row.depth, row.text.unwrap_or_default()))
            .collect()
    })
}

fn row(depth: usize, text: &str) -> (usize, String) {
    (depth, text.into())
}

fn handle(cx: &mut VisualTestContext, block: BlockId) -> Point<gpui::Pixels> {
    let selector: &'static str = Box::leak(format!("block-handle-{block}").into_boxed_str());
    cx.debug_bounds(selector).unwrap().center()
}

/// Where `block`'s row was drawn: its left edge, top and bottom.
fn drawn(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
) -> gpui::Bounds<gpui::Pixels> {
    shell.update(cx, |shell, _| {
        shell
            .blocks
            .drawn
            .0
            .borrow()
            .iter()
            .find(|(b, _)| *b == block)
            .map(|(_, bounds)| *bounds)
            .unwrap()
    })
}

/// Presses on `from` and moves to `to` in small steps, as a hand would, without letting go.
fn drag_to(cx: &mut VisualTestContext, from: Point<gpui::Pixels>, to: Point<gpui::Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=8u8 {
        let t = f32::from(step) / 8.0;
        let at = point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
        draw(cx);
    }
}

fn release(cx: &mut VisualTestContext, at: Point<gpui::Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

#[gpui::test]
fn dragging_a_handle_shows_a_line_and_drops_the_block_there(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &words(&["a", "b", "c"]));
    let a = drawn(&shell, cx, ids[0]);
    let to = point(a.left() + px(4.0), a.top() + px(3.0));
    let from = handle(cx, ids[2]);
    drag_to(cx, from, to);
    assert!(cx.debug_bounds("block-drop-line").is_some(), "the line shows");
    let (faded, drop) = shell.update(cx, |shell, _| {
        let drag = shell.blocks.dragging.as_ref().unwrap();
        (drag.faded.contains(&ids[2]), drag.drop.unwrap())
    });
    assert!(faded, "the dragged block is faded");
    assert_eq!((drop.below, drop.depth), (None, 0), "above the first block");
    release(cx, to);
    assert_eq!(outline(&shell, cx), vec![row(0, "c"), row(0, "a"), row(0, "b")]);
    assert!(shell.update(cx, |shell, _| shell.blocks.dragging.is_none()));
    assert!(cx.debug_bounds("block-drop-line").is_none());
    assert!(
        shell.update(cx, |shell, _| shell.block_menu().is_none()),
        "a drag doesn't open the menu"
    );
}

#[gpui::test]
fn moving_right_nests_the_block(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &words(&["a", "b", "c"]));
    let a = drawn(&shell, cx, ids[0]);
    let to = point(a.left() + px(super::INDENT_PX + 6.0), a.bottom() - px(3.0));
    let from = handle(cx, ids[2]);
    drag_to(cx, from, to);
    let drop = shell.update(cx, |shell, _| {
        shell.blocks.dragging.as_ref().unwrap().drop.unwrap()
    });
    assert_eq!(
        (drop.below, drop.depth, drop.parent),
        (Some(ids[0]), 1, Some(ids[0]))
    );
    let line = cx.debug_bounds("block-drop-line").unwrap();
    assert!(
        line.left() >= a.left() + px(super::INDENT_PX),
        "indented to show nesting"
    );
    release(cx, to);
    assert_eq!(outline(&shell, cx), vec![row(0, "a"), row(1, "c"), row(0, "b")]);
}

#[gpui::test]
fn letting_go_off_the_page_moves_nothing(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &words(&["a", "b"]));
    let a = drawn(&shell, cx, ids[0]);
    let from = handle(cx, ids[1]);
    drag_to(cx, from, point(a.left(), a.top() + px(2.0)));
    // The sidebar, left of the page.
    let away = point(px(20.0), a.top());
    cx.simulate_mouse_move(away, MouseButton::Left, Modifiers::none());
    release(cx, away);
    assert_eq!(outline(&shell, cx), vec![row(0, "a"), row(0, "b")]);
    assert!(shell.update(cx, |shell, _| shell.blocks.dragging.is_none()));
}

#[gpui::test]
fn a_click_on_the_handle_still_opens_the_menu(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &words(&["a"]));
    let at = handle(cx, ids[0]);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(shell.update(cx, |shell, _| shell.block_menu().is_some()));
}

#[gpui::test]
fn holding_near_the_bottom_edge_scrolls_the_page(cx: &mut TestAppContext) {
    let texts: Vec<String> = (0..120).map(|i| format!("line {i}")).collect();
    let (shell, cx, ids) = open(cx, &texts);
    let viewport = shell.update(cx, |shell, _| shell.blocks.list.state.viewport_bounds());
    let bottom = point(viewport.center().x, viewport.bottom() - px(4.0));
    let from = handle(cx, ids[0]);
    drag_to(cx, from, bottom);
    let top_before = shell.update(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    draw(cx);
    let top_after = shell.update(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    assert!(
        top_after > top_before,
        "scrolled from item {top_before} to {top_after}"
    );

    // Away from the edge, scrolling stops.
    cx.simulate_mouse_move(viewport.center(), MouseButton::Left, Modifiers::none());
    draw(cx);
    let held = shell.update(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(500));
    draw(cx);
    let still = shell.update(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    assert_eq!(still, held);

    release(cx, viewport.center());
    let moved = outline(&shell, cx);
    assert_ne!(moved[0].1, "line 0", "dropped further down the page");
    assert_eq!(moved.len(), 120);
}

#[test]
fn scrolling_is_faster_closer_to_the_edge_and_none_in_the_middle() {
    assert_eq!(scroll_speed(300.0, 0.0, 600.0), 0.0);
    assert!(scroll_speed(2.0, 0.0, 600.0) < scroll_speed(30.0, 0.0, 600.0));
    assert!(scroll_speed(30.0, 0.0, 600.0) < 0.0, "up near the top");
    assert!(scroll_speed(598.0, 0.0, 600.0) > scroll_speed(570.0, 0.0, 600.0));
    assert!(scroll_speed(570.0, 0.0, 600.0) > 0.0, "down near the bottom");
    assert_eq!(
        scroll_speed(-50.0, 0.0, 600.0),
        -super::MAX_SCROLL_PX,
        "full speed past the edge"
    );
}

#[test]
fn dragged_blocks_fade_with_everything_inside_them() {
    use leaf_editor::{BlockRow, BlockStyle};
    let ids: Vec<BlockId> = (0..4).map(|_| BlockId::new()).collect();
    let rows: Vec<BlockRow> = [0, 1, 2, 0]
        .iter()
        .zip(&ids)
        .map(|(&depth, &id)| BlockRow {
            id,
            depth,
            style: BlockStyle::Paragraph,
            text: Some(String::new()),
            spans: None,
        })
        .collect();
    let faded = with_insides(&rows, &[ids[0]]);
    assert_eq!(faded.len(), 3);
    assert!(!faded.contains(&ids[3]));
}
