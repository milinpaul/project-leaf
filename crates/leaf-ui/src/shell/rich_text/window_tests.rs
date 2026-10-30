//! The text view in a (test) window: its painted layout answers the editor's line questions
//! consistently with what was drawn, and Up/Down move between the painted lines (E07-S02).

use gpui::{TestAppContext, VisualTestContext};
use leaf_editor::{LineLayout, Vertical};
use leaf_model::Caret;

use crate::shell::Shell;

const LONG: &str = "Leaf keeps every page on your own computer, saves as you type, and never asks for an \
account. This sentence is long on purpose, so that it wraps onto several lines in the page column, which \
lets the test compare the editor's idea of lines with the lines that were actually painted.";

fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, leaf_core::BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let first = page.ensure_block().unwrap();
        page.set_text(first, "short").unwrap();
        let long = page.enter(first).unwrap();
        page.set_text(long, LONG).unwrap();
        let last = page.enter(long).unwrap();
        page.set_text(last, "after").unwrap();
        long
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx, block)
}

#[gpui::test]
fn the_painted_layout_wraps_and_round_trips_every_position(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    let lines = painted.line_count(block);
    assert!(lines > 1, "the long paragraph wraps: {lines} line(s)");

    let len = LONG.chars().count();
    let mut last_line = 0;
    for offset in 0..=len {
        let line = painted.line_of(block, offset);
        assert!(
            line >= last_line && line < lines,
            "lines run in order: {offset} on {line}"
        );
        last_line = line;
        let x = painted.offset_to_x(block, offset);
        assert_eq!(
            painted.x_to_offset(block, line, x),
            offset,
            "offset {offset} -> x {x} on line {line} -> back"
        );
    }
    assert_eq!(last_line, lines - 1, "the end is on the last line");
}

#[gpui::test]
fn up_and_down_move_between_the_painted_lines(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    let lines = painted.line_count(block);
    shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        page.set_caret(Caret { block, at: 3 }).unwrap();
        let rows = page.rows().unwrap();
        // Down through each wrapped line of the paragraph, then into the next block.
        for line in 1..lines {
            let caret = page.move_vertically(Vertical::Down, &painted).unwrap().unwrap();
            assert_eq!((caret.block, painted.line_of(block, caret.at)), (block, line));
        }
        let below = page.move_vertically(Vertical::Down, &painted).unwrap().unwrap();
        assert_eq!(below.block, rows[2].id, "past the last line: the next block");
        // And back up into the paragraph's last line.
        let up = page.move_vertically(Vertical::Up, &painted).unwrap().unwrap();
        assert_eq!((up.block, painted.line_of(block, up.at)), (block, lines - 1));
    });
}

#[gpui::test]
fn blocks_not_on_screen_answer_as_one_line(cx: &mut TestAppContext) {
    let (shell, cx, _) = open(cx);
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    let elsewhere = leaf_core::BlockId::new();
    assert_eq!(painted.line_count(elsewhere), 1);
    assert_eq!(painted.line_of(elsewhere, 5), 0);
    assert!(painted.offset_to_x(elsewhere, 5).abs() < f32::EPSILON);
    assert_eq!(painted.x_to_offset(elsewhere, 0, 100.0), 0);
    assert_eq!(
        painted.offset_at(elsewhere, gpui::point(gpui::px(1.0), gpui::px(1.0))),
        None
    );
}

#[gpui::test]
fn the_caret_blinks_while_the_text_view_draws_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    let on = |shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.blocks.caret_on)
    };
    // No caret yet: nothing blinks.
    assert!(on(&shell, cx));
    // The caret is drawn while the page has the keyboard.
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block, at: 4 }, window, cx);
        });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(on(&shell, cx), "it starts visible");
    cx.executor().advance_clock(std::time::Duration::from_millis(540));
    cx.run_until_parked();
    assert!(!on(&shell, cx), "then hides");
    cx.executor().advance_clock(std::time::Duration::from_millis(540));
    cx.run_until_parked();
    assert!(on(&shell, cx), "and shows again");
    // The keyboard moves to the title: the page stops drawing (and blinking) its caret.
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.focus_title(window, cx)));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.executor().advance_clock(std::time::Duration::from_millis(540));
    cx.run_until_parked();
    assert!(on(&shell, cx), "stopped, left visible");
}

/// A paragraph as long as a pasted meeting transcript: about 38,000 characters, no line breaks.
fn transcript() -> String {
    let words = [
        "so", "the", "partner", "maps", "their", "fields", "and", "then", "we", "review", "it.",
    ];
    let mut text = String::new();
    let mut i = 0;
    while text.len() < 38_000 {
        text.push_str(words[i % words.len()]);
        text.push(' ');
        i += 7;
    }
    text
}

#[gpui::test]
fn clicking_and_selecting_in_a_very_long_paragraph_stay_quick(cx: &mut TestAppContext) {
    use std::time::{Duration, Instant};

    use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent};

    let (shell, cx, block) = open(cx);
    let text = transcript();
    shell.update(cx, |shell, _| {
        shell
            .editor
            .open_page()
            .unwrap()
            .unwrap()
            .set_text(block, &text)
            .unwrap();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
    assert!(painted.line_count(block) > 10, "it wraps over many lines");

    // A click into the paragraph and a drag across it: each turns the pointer into a text position.
    let started = Instant::now();
    let at = painted.caret_bounds(block, 500).unwrap().0 + gpui::point(gpui::px(1.0), gpui::px(2.0));
    let to = painted.caret_bounds(block, 900).unwrap().0 + gpui::point(gpui::px(1.0), gpui::px(2.0));
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseMoveEvent {
        position: to,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers::none(),
    });
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: to,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    let took = started.elapsed();
    let selection = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert!(
        matches!(selection, Some(leaf_editor::Selection::Text { .. })),
        "the drag selected text: {selection:?}"
    );
    // Generous: a debug build on a slow CI machine. Before the fix this took minutes.
    assert!(took < Duration::from_secs(3), "click and drag took {took:?}");
}

#[gpui::test]
fn every_boundary_is_where_gpui_draws_it(cx: &mut TestAppContext) {
    let (shell, cx, block) = open(cx);
    // Wrapped lines, formatting, and a hard line break inside the block, with non-Latin and accented text.
    let texts = [LONG.to_owned(), format!("{LONG}\nZürich 東京 then more {LONG}")];
    for text in texts {
        shell.update(cx, |shell, _| {
            let page = shell.editor.open_page().unwrap().unwrap();
            page.set_text(block, &text).unwrap();
            // Bold and code in the middle split the text into several runs of glyphs.
            page.select(block, 10..40).unwrap();
            page.toggle_mark(leaf_model::MarkKind::Bold).unwrap();
            page.select(block, 120..150).unwrap();
            page.toggle_mark(leaf_model::MarkKind::Code).unwrap();
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let painted = shell.read_with(cx, |shell, _| shell.blocks.painted.clone());
        for offset in 0..=text.chars().count() {
            assert_eq!(
                painted.drawn_at(block, offset),
                painted.gpui_position(block, offset),
                "boundary {offset}"
            );
        }
    }
}
