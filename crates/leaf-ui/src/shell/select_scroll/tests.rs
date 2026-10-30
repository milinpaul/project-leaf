//! Scrolling while drag-selecting text (E17-S02), in a (test) window.

use std::time::Duration;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};
use leaf_core::BlockId;
use leaf_editor::{PageEditor, Selection};
use leaf_model::Caret;

use crate::shell::Shell;

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, Vec<BlockId>) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let ids = shell.update(cx, |shell, _| {
        let page: &mut PageEditor = shell.editor.open_page().unwrap().unwrap();
        let mut ids = vec![page.ensure_block().unwrap()];
        page.set_text(ids[0], "block 0").unwrap();
        for i in 1..200 {
            let next = page.enter(*ids.last().unwrap()).unwrap();
            page.set_text(next, &format!("block {i}")).unwrap();
            ids.push(next);
        }
        ids
    });
    draw(cx);
    (shell, cx, ids)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn wait(cx: &mut VisualTestContext, ms: u64) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
    draw(cx);
}

fn scroll_top(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> usize {
    shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    })
}

/// Where block `block`'s text starts, as drawn.
fn text_of(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> Point<Pixels> {
    let (top, height) = shell.read_with(cx, |shell, _| {
        shell.blocks.painted.caret_bounds(block, 0).unwrap()
    });
    point(top.x + px(0.5), top.y + height / 2.0)
}

fn drag_to(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_event(MouseMoveEvent {
        position: at,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    draw(cx);
}

/// The selection's far end: the block the drag has reached.
fn selection_end(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Option<BlockId> {
    shell.update(cx, |shell, _| match shell.page().unwrap().selection().unwrap() {
        Some(Selection::Text { end, .. }) => Some(end.block),
        Some(Selection::Blocks(blocks)) => blocks.last().copied(),
        _ => None,
    })
}

#[gpui::test]
fn dragging_a_selection_past_the_bottom_scrolls_and_keeps_selecting(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx);
    let start = text_of(&shell, cx, ids[1]);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    let view = shell.read_with(cx, |shell, _| shell.blocks.list.state.viewport_bounds());
    let last_shown = shell.read_with(cx, |shell, _| {
        ids.iter()
            .rposition(|&id| shell.blocks.painted.contains(id))
            .unwrap()
    });

    // Past the bottom edge, then hold still: the page keeps scrolling and the selection keeps growing.
    drag_to(cx, point(start.x, view.bottom() + px(40.0)));
    wait(cx, 600);
    let top = scroll_top(&shell, cx);
    assert!(top > 1, "the page scrolled down: item {top}");
    let end = selection_end(&shell, cx).expect("a selection");
    let reached = ids.iter().position(|&id| id == end).unwrap();
    assert!(
        reached > last_shown,
        "the selection reached block {reached}, past the {last_shown} first shown"
    );

    // Further out is faster.
    let before = scroll_top(&shell, cx);
    wait(cx, 200);
    let slow = scroll_top(&shell, cx) - before;
    drag_to(cx, point(start.x, view.bottom() + px(200.0)));
    let before = scroll_top(&shell, cx);
    wait(cx, 200);
    let fast = scroll_top(&shell, cx) - before;
    assert!(
        fast > slow,
        "{fast} rows a moment further out, {slow} just past the edge"
    );

    // Letting go stops it.
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: point(start.x, view.bottom() + px(200.0)),
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    draw(cx);
    let stopped = scroll_top(&shell, cx);
    wait(cx, 300);
    assert_eq!(
        scroll_top(&shell, cx),
        stopped,
        "no more scrolling once the button is up"
    );
}

#[gpui::test]
fn dragging_back_inside_the_page_stops_the_scrolling(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx);
    let start = text_of(&shell, cx, ids[1]);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: start,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    let view = shell.read_with(cx, |shell, _| shell.blocks.list.state.viewport_bounds());
    drag_to(cx, point(start.x, view.bottom() + px(20.0)));
    wait(cx, 200);
    drag_to(cx, view.center());
    let held = scroll_top(&shell, cx);
    wait(cx, 300);
    assert_eq!(scroll_top(&shell, cx), held, "still in the middle of the page");
}

#[gpui::test]
fn shift_down_keeps_the_end_of_the_selection_in_view(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block: ids[0], at: 0 }, window, cx);
        });
    });
    draw(cx);
    for _ in 0..80 {
        cx.simulate_keystrokes("shift-down");
        draw(cx);
    }
    let end = selection_end(&shell, cx).expect("a selection");
    let i = ids.iter().position(|&id| id == end).unwrap();
    assert!(i >= 60, "the selection went down the page: block {i}");
    shell.read_with(cx, |shell, _| {
        let state = &shell.blocks.list.state;
        let row = state
            .bounds_for_item(1 + i)
            .expect("the selection's end is drawn");
        let view = state.viewport_bounds();
        assert!(
            row.top() >= view.top() && row.bottom() <= view.bottom(),
            "{row:?} in {view:?}"
        );
    });
}
