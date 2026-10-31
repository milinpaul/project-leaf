//! The page's slim scrollbar in a (test) window (E07-S14): dragging its thumb scrolls the page.

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, TestAppContext, VisualTestContext,
    point, px,
};
use leaf_editor::PageEditor;

use crate::shell::Shell;

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

fn scrolled(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
    shell.read_with(cx, |shell, _| {
        f32::from(shell.blocks.list.state.scroll_px_offset_for_scrollbar().y).abs()
    })
}

#[gpui::test]
fn dragging_the_thumb_scrolls_the_page(cx: &mut TestAppContext) {
    let (shell, cx) = open(cx);
    let view = shell.read_with(cx, |shell, _| shell.blocks.list.state.viewport_bounds());
    assert_eq!(scrolled(&shell, cx), 0.0, "starts at the top");
    // The thumb sits at the top of the page's right edge; drag it most of the way down.
    let grab = point(view.right() - px(5.0), view.top() + px(12.0));
    let to = point(grab.x, view.bottom() - px(12.0));
    cx.simulate_event(MouseMoveEvent {
        position: grab,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    draw(cx);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: grab,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    for step in 1..=10_u8 {
        let y = grab.y + (to.y - grab.y) * (f32::from(step) / 10.0);
        cx.simulate_event(MouseMoveEvent {
            position: point(grab.x, y),
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::none(),
        });
        draw(cx);
    }
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: to,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    draw(cx);
    let moved = scrolled(&shell, cx);
    assert!(moved > 300.0, "dragging the thumb down scrolls the page: {moved}");
    // The pointer was on the scrollbar, not the text: no selection was started.
    assert!(shell.read_with(cx, |shell, _| shell.blocks.drag.is_none()));
    let selected = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert!(selected.is_none(), "{selected:?}");
}
