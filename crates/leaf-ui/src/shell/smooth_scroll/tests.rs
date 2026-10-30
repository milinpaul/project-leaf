//! The wheel glide (E17-S01): its ease, and the page in a (test) window.

use std::time::Duration;

use gpui::{Modifiers, ScrollDelta, ScrollEvent, TestAppContext, VisualTestContext, point, px};
use leaf_editor::PageEditor;

use super::{Glide, LINE_PX};
use crate::shell::Shell;

#[test]
fn the_glide_eases_out_and_adds_up_to_the_distance() {
    let mut glide = Glide::default();
    glide.add(120.0);
    let first = glide.step(Duration::from_millis(8));
    let second = glide.step(Duration::from_millis(8));
    assert!(
        first > 0.0 && first < 120.0,
        "a first share, not the whole way: {first}"
    );
    assert!(second < first, "slowing down: {second} after {first}");
    let mut total = first + second;
    let mut steps = 2;
    while !glide.is_idle() {
        total += glide.step(Duration::from_millis(8));
        steps += 1;
        assert!(steps < 100, "the glide ends");
    }
    assert!((total - 120.0).abs() < 1e-3, "exactly the distance: {total}");
    assert!(steps * 8 <= 300, "done in about 150 ms: {} ms", steps * 8);
}

#[test]
fn notches_in_a_row_add_up_in_either_direction() {
    let mut glide = Glide::default();
    glide.add(40.0);
    let mut total = glide.step(Duration::from_millis(8));
    glide.add(40.0);
    glide.add(-20.0);
    while !glide.is_idle() {
        total += glide.step(Duration::from_millis(8));
    }
    assert!((total - 60.0).abs() < 1e-3, "{total}");
    assert_eq!(glide.step(Duration::from_millis(8)), 0.0, "idle stays still");
}

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    shell.update(cx, |shell, _| {
        let page: &mut PageEditor = shell.editor.open_page().unwrap().unwrap();
        let mut block = page.ensure_block().unwrap();
        for i in 0..200 {
            page.set_text(block, &format!("block {i}")).unwrap();
            block = page.enter(block).unwrap();
        }
    });
    draw(cx);
    (shell, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// How far down the page is scrolled, in pixels.
fn scrolled(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.read_with(cx, |shell, _| {
        f32::from(shell.blocks.list.state.scroll_px_offset_for_scrollbar().y).abs()
    })
}

fn wheel(cx: &mut VisualTestContext, shell: &gpui::Entity<Shell>, delta: ScrollDelta) {
    let at = shell.read_with(cx, |shell, _| shell.blocks.list.state.viewport_bounds().center());
    cx.simulate_event(ScrollEvent {
        position: at,
        delta,
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    draw(cx);
}

#[gpui::test]
fn a_wheel_notch_glides_instead_of_jumping(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    let start = scrolled(&shell, cx);
    // Three notches down.
    wheel(cx, &shell, ScrollDelta::Lines(point(0.0, -3.0)));
    let right_away = scrolled(&shell, cx) - start;
    assert!(right_away < 1.0, "no jump on the notch itself: {right_away}");
    cx.executor().advance_clock(Duration::from_millis(16));
    cx.run_until_parked();
    draw(cx);
    let early = scrolled(&shell, cx) - start;
    assert!(
        early > 0.0 && early < 3.0 * LINE_PX,
        "part of the way after a frame: {early}"
    );
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    draw(cx);
    let done = scrolled(&shell, cx) - start;
    assert!(
        (done - 3.0 * LINE_PX).abs() < 1.0,
        "the whole way once it settles: {done}"
    );
}

#[gpui::test]
fn trackpad_pixels_still_move_the_page_at_once(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    let start = scrolled(&shell, cx);
    wheel(cx, &shell, ScrollDelta::Pixels(point(px(0.0), px(-90.0))));
    let moved = scrolled(&shell, cx) - start;
    assert!((moved - 90.0).abs() < 1.0, "{moved}");
}

#[gpui::test]
fn scrolling_through_rows_not_measured_yet_never_jumps_back(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    let mut last = (0, 0.0);
    for _ in 0..80 {
        wheel(cx, &shell, ScrollDelta::Pixels(point(px(0.0), px(-60.0))));
        let top = shell.read_with(cx, |shell, _| shell.blocks.list.state.logical_scroll_top());
        let now = (top.item_ix, f32::from(top.offset_in_item));
        assert!(
            now.0 > last.0 || (now.0 == last.0 && now.1 >= last.1),
            "the top of the window only moves down: {now:?} after {last:?}"
        );
        last = now;
    }
    assert!(last.0 > 50, "well down the page: {last:?}");
}
