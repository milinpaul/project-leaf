//! The page list in a real (test) window: a 10,000-block page builds only the rows on screen
//! (E07-S01).

use gpui::{ListOffset, TestAppContext, VisualTestContext, px};
use leaf_editor::{BlockStyle, PageEditor};

use crate::shell::Shell;

const BLOCKS: usize = 10_000;

fn open_shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A new page with `count` blocks: numbered items, every tenth a nested bullet, the rest paragraphs.
fn big_page(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, count: usize) {
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    shell.update(cx, |shell, _| {
        let page: &mut PageEditor = shell.editor.open_page().unwrap().unwrap();
        let mut block = page.ensure_block().unwrap();
        page.set_text(block, "block 0").unwrap();
        for i in 1..count {
            block = page.enter(block).unwrap();
            page.set_text(block, &format!("block {i}")).unwrap();
            if i % 10 == 0 {
                page.turn_into(block, leaf_model::BlockKind::BulletedList)
                    .unwrap();
            }
        }
    });
}

#[gpui::test]
fn a_ten_thousand_block_page_builds_only_the_rows_on_screen(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, BLOCKS);
    draw(cx);
    let (items, painted) = shell.read_with(cx, |shell, _| {
        (shell.blocks.list.state.item_count(), shell.blocks.painted.len())
    });
    assert_eq!(items, BLOCKS + 1, "the title plus every block is in the list");
    assert!(
        painted > 0 && painted < 150,
        "only the rows on screen (plus a little overdraw) were laid out: {painted}"
    );

    // Scroll to the middle: rows there get built, and still only a screenful.
    shell.update(cx, |shell, _| {
        shell.blocks.list.state.scroll_to(ListOffset {
            item_ix: 5_000,
            offset_in_item: px(0.0),
        });
    });
    draw(cx);
    let (built_middle, painted) = shell.read_with(cx, |shell, _| {
        let middle = shell.blocks.list.rows[5_000].id;
        (shell.blocks.painted.contains(middle), shell.blocks.painted.len())
    });
    assert!(built_middle, "the row scrolled to is on screen");
    assert!(painted < 150, "still only a screenful: {painted}");
}

#[gpui::test]
fn editing_keeps_the_list_in_step_with_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 30);
    draw(cx);
    // Delete a block and add one: the list follows without being rebuilt from scratch.
    shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let rows = page.rows().unwrap();
        page.delete_block(rows[3].id).unwrap();
        page.enter(rows[10].id).unwrap();
    });
    draw(cx);
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.blocks.list.state.item_count(), 31);
        assert_eq!(shell.blocks.list.rows.len(), 30);
        assert!(
            matches!(shell.blocks.list.rows[9].style, BlockStyle::Bulleted),
            "row 10 of the page is a bullet"
        );
    });
}

#[gpui::test]
fn a_scroll_asked_for_right_after_opening_a_page_survives_the_reset(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 2_000);
    draw(cx);
    // Open another page and come back, asking for block 1,500 before the next frame.
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    draw(cx);
    let first = shell.read_with(cx, |shell, _| shell.editor().sidebar().unwrap()[0].page);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open(first, window, cx);
            shell.scroll_page_to(1_500, cx);
        });
    });
    draw(cx);
    let top = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.logical_scroll_top().item_ix
    });
    assert_eq!(top, 1 + 1_500, "the title is item 0");
}

/// E17-S06: scrolled down a long page, clicking a block left the page at the top.
#[gpui::test]
fn clicking_a_block_keeps_the_scroll_position(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 2_000);
    draw(cx);
    // Scroll down the way the owner does: with the trackpad, a frame at a time.
    let inside = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.bounds_for_item(1).unwrap().center()
    });
    for _ in 0..60 {
        cx.simulate_event(gpui::ScrollEvent {
            position: inside,
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-400.0))),
            ..Default::default()
        });
        draw(cx);
    }
    let (before, target) = shell.read_with(cx, |shell, _| {
        let state = &shell.blocks.list.state;
        let top = state.logical_scroll_top();
        assert!(
            top.item_ix > 100,
            "the wheel scrolled well down the page: {top:?}"
        );
        let bounds = state.bounds_for_item(top.item_ix + 3).unwrap();
        (top, bounds.center())
    });
    cx.simulate_click(target, gpui::Modifiers::none());
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();
    draw(cx);
    let (after, caret_block, clicked) = shell.update(cx, |shell, _| {
        let after = shell.blocks.list.state.logical_scroll_top();
        let clicked = shell.blocks.list.rows[before.item_ix + 3 - 1].id;
        (after, shell.caret_block(), clicked)
    });
    assert_eq!(
        caret_block,
        Some(clicked),
        "the click put the caret in the block under the mouse"
    );
    assert_eq!(
        after.item_ix, before.item_ix,
        "the page stayed where it was scrolled to"
    );
}

/// E17-S06: the same at the end of a page a few screens long.
#[gpui::test]
fn clicking_a_block_at_the_end_of_a_short_page_keeps_the_scroll_position(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 80);
    draw(cx);
    // Scroll down the way the owner does: with the trackpad, a frame at a time.
    let inside = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.bounds_for_item(1).unwrap().center()
    });
    for _ in 0..60 {
        cx.simulate_event(gpui::ScrollEvent {
            position: inside,
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-400.0))),
            ..Default::default()
        });
        draw(cx);
    }
    let (before, target) = shell.read_with(cx, |shell, _| {
        let state = &shell.blocks.list.state;
        let top = state.logical_scroll_top();
        assert!(
            top.item_ix > 20,
            "the wheel scrolled to the end of the page: {top:?}"
        );
        let bounds = state.bounds_for_item(top.item_ix + 3).unwrap();
        (top, bounds.center())
    });
    cx.simulate_click(target, gpui::Modifiers::none());
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();
    draw(cx);
    let (after, caret_block, clicked) = shell.update(cx, |shell, _| {
        let after = shell.blocks.list.state.logical_scroll_top();
        let clicked = shell.blocks.list.rows[before.item_ix + 3 - 1].id;
        (after, shell.caret_block(), clicked)
    });
    assert_eq!(
        caret_block,
        Some(clicked),
        "the click put the caret in the block under the mouse"
    );
    assert_eq!(
        after.item_ix, before.item_ix,
        "the page stayed where it was scrolled to"
    );
}

/// E17-S06, the real cause: on macOS every click goes to the system's text input first, which calls
/// `unmark_text` before the page sees the click. With the caret still near the top, that scrolled the
/// page back up to it.
#[gpui::test]
fn the_click_time_unmark_text_call_keeps_the_scroll_position(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 2_000);
    draw(cx);
    // The caret stays in the first block while the page scrolls away from it.
    let first = shell.read_with(cx, |shell, _| shell.blocks.list.rows[0].id);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(leaf_model::Caret { block: first, at: 0 }, window, cx);
        });
    });
    draw(cx);
    let inside = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.bounds_for_item(1).unwrap().center()
    });
    for _ in 0..60 {
        cx.simulate_event(gpui::ScrollEvent {
            position: inside,
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-400.0))),
            ..Default::default()
        });
        draw(cx);
    }
    let before = shell.read_with(cx, |shell, _| shell.blocks.list.state.logical_scroll_top());
    assert!(
        before.item_ix > 100,
        "the wheel scrolled well down the page: {before:?}"
    );
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            gpui::EntityInputHandler::unmark_text(shell, window, cx);
        });
    });
    draw(cx);
    let after = shell.read_with(cx, |shell, _| shell.blocks.list.state.logical_scroll_top());
    assert_eq!(
        after.item_ix, before.item_ix,
        "the page stayed where it was scrolled to"
    );
}

/// E17-S06, the cover case: a page with a cover, scrolled a little so the top of the window is still
/// inside the tall first row (cover and title). Clicking a block on screen jumped to the very top,
/// because GPUI's `scroll_to_reveal_item` lines a row up with the bottom even when it is in view.
#[gpui::test]
fn clicking_a_block_under_a_cover_keeps_the_scroll_position(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 200);
    shell.update(cx, |shell, _| {
        let page = shell.editor.selected().unwrap();
        shell
            .editor
            .set_cover(page, Some(leaf_editor::BuiltinCover::Forest))
            .unwrap();
    });
    draw(cx);
    let inside = shell.read_with(cx, |shell, _| {
        shell.blocks.list.state.bounds_for_item(1).unwrap().center()
    });
    cx.simulate_event(gpui::ScrollEvent {
        position: inside,
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-120.0))),
        ..Default::default()
    });
    draw(cx);
    let (before, target) = shell.read_with(cx, |shell, _| {
        let state = &shell.blocks.list.state;
        let top = state.logical_scroll_top();
        assert_eq!(top.item_ix, 0, "still inside the cover and title: {top:?}");
        assert!(top.offset_in_item > px(0.0), "scrolled a little: {top:?}");
        (top, state.bounds_for_item(6).unwrap().center())
    });
    cx.simulate_click(target, gpui::Modifiers::none());
    draw(cx);
    let after = shell.read_with(cx, |shell, _| shell.blocks.list.state.logical_scroll_top());
    assert_eq!(after, before, "the page stayed where it was scrolled to");
}

/// The other side of E17-S06: moving the caret off the bottom of the window still brings the page
/// along, with or without a cover.
#[gpui::test]
fn the_caret_moving_off_screen_still_scrolls_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open_shell(cx);
    big_page(&shell, cx, 200);
    shell.update(cx, |shell, _| {
        let page = shell.editor.selected().unwrap();
        shell
            .editor
            .set_cover(page, Some(leaf_editor::BuiltinCover::Forest))
            .unwrap();
    });
    draw(cx);
    let first = shell.read_with(cx, |shell, _| shell.blocks.list.rows[0].id);
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(leaf_model::Caret { block: first, at: 0 }, window, cx);
        });
    });
    draw(cx);
    for _ in 0..60 {
        cx.simulate_keystrokes("down");
        draw(cx);
    }
    shell.update(cx, |shell, _| {
        let caret = shell.caret_block().unwrap();
        let i = shell
            .blocks
            .list
            .rows
            .iter()
            .position(|row| row.id == caret)
            .unwrap();
        assert!(i >= 50, "the caret went down the page: block {i}");
        let state = &shell.blocks.list.state;
        let row = state.bounds_for_item(1 + i).expect("the caret's row is drawn");
        let view = state.viewport_bounds();
        assert!(
            row.top() >= view.top() && row.bottom() <= view.bottom(),
            "the caret's row is on screen: {row:?} in {view:?}"
        );
    });
}
