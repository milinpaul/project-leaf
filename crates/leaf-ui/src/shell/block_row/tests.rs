//! A quote with several lines in a (test) window (E17-S13): its bar runs down all of them.

use gpui::{TestAppContext, VisualTestContext};
use leaf_model::{BlockKind, Caret};

use crate::shell::Shell;

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui::test]
fn enter_in_a_quote_adds_a_line_and_the_bar_covers_every_line(cx: &mut TestAppContext) {
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
    cx.simulate_input("> To be");
    draw(cx);
    let one_line = cx
        .debug_bounds(Box::leak(format!("quote-bar-{block}").into_boxed_str()))
        .unwrap();

    cx.simulate_keystrokes("enter");
    cx.simulate_input("or not");
    draw(cx);
    let (kind, text) = shell.update(cx, |shell, _| {
        let doc = shell.page().unwrap().doc();
        (doc.kind(block).unwrap(), doc.plain_text(block).unwrap().unwrap())
    });
    assert_eq!(kind, BlockKind::Quote);
    assert_eq!(text, "To be\nor not", "one quote, two lines");
    let two_lines = cx
        .debug_bounds(Box::leak(format!("quote-bar-{block}").into_boxed_str()))
        .unwrap();
    assert!(
        two_lines.size.height > one_line.size.height * 1.5,
        "the bar grows with the quote: {:?} then {:?}",
        one_line.size.height,
        two_lines.size.height
    );
}
