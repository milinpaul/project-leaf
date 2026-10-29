//! The slash menu's popover in a (test) window (E07-S06), and where it opens.

use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, TestAppContext, VisualTestContext, px};
use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret};

use super::{Side, place};
use crate::shell::Shell;

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
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
    (shell, cx, block)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn kind(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(block).unwrap())
}

fn text(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> String {
    shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().plain_text(block).unwrap().unwrap()
    })
}

#[gpui::test]
fn typing_a_slash_shows_the_popover_and_the_keyboard_drives_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    assert!(cx.debug_bounds("slash-menu").is_none());
    cx.simulate_input("/");
    draw(cx);
    let menu = cx.debug_bounds("slash-menu").expect("open under the slash");
    let line = shell.read_with(cx, |shell, _| {
        shell.blocks.painted.caret_bounds(block, 0).unwrap()
    });
    assert!(menu.top() >= line.0.y + line.1, "below the line");
    assert!(
        cx.debug_bounds("slash-item-9").is_some(),
        "every entry, divider last"
    );

    // Typing filters; Down then Enter picks the second match.
    cx.simulate_input("head");
    draw(cx);
    assert!(
        cx.debug_bounds("slash-item-3").is_none(),
        "only the headings are left"
    );
    cx.simulate_keystrokes("down enter");
    draw(cx);
    assert!(cx.debug_bounds("slash-menu").is_none());
    assert_eq!(kind(&shell, cx, block), BlockKind::Heading { level: 2 });
    assert_eq!(text(&shell, cx, block), "");

    // Escape closes it and leaves what was typed.
    cx.simulate_input("/zzz");
    draw(cx);
    assert!(cx.debug_bounds("slash-item-0").is_none());
    assert!(
        cx.debug_bounds("slash-menu").is_some(),
        "says there are no results"
    );
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("slash-menu").is_none());
    assert_eq!(text(&shell, cx, block), "/zzz");
}

#[gpui::test]
fn the_mouse_highlights_and_picks_an_entry(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    cx.simulate_input("/");
    draw(cx);
    let quote = cx.debug_bounds("slash-item-7").unwrap().center();
    cx.simulate_event(MouseMoveEvent {
        position: quote,
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    draw(cx);
    let selected = shell.update(cx, |shell, _| {
        shell.page().unwrap().slash_menu().unwrap().unwrap().selected
    });
    assert_eq!(selected, 7);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: quote,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    assert_eq!(kind(&shell, cx, block), BlockKind::Quote);
    // The page still has the keyboard.
    cx.simulate_input("x");
    draw(cx);
    assert_eq!(text(&shell, cx, block), "x");
}

#[test]
fn it_opens_below_the_line_unless_there_is_no_room() {
    let window = px(800.0);
    let (top, bottom, height) = (px(100.0), px(124.0), px(300.0));
    assert_eq!(place(top, bottom, height, window), (Side::Below, px(128.0)));
    // Near the bottom edge it goes above.
    let (top, bottom) = (px(700.0), px(724.0));
    assert_eq!(place(top, bottom, height, window), (Side::Above, px(696.0)));
    // Room on neither side: the roomier one.
    let short = px(400.0);
    assert_eq!(place(px(100.0), px(124.0), height, short).0, Side::Below);
    assert_eq!(place(px(250.0), px(274.0), height, short).0, Side::Above);
}
