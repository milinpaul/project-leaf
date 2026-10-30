//! The page taking the keyboard, input methods and the mouse (E07-S03), in a (test) window.
//!
//! Input methods are driven the way the platform drives them: a run of "mark this draft" calls while
//! the user composes, then one "insert" with the chosen text.

use gpui::{
    EntityInputHandler as _, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, VisualTestContext,
};
use leaf_core::BlockId;
use leaf_model::Caret;

use super::{from_utf16, to_utf16};
use crate::shell::Shell;

fn open<'a>(
    cx: &'a mut TestAppContext,
    texts: &[&str],
) -> (gpui::Entity<Shell>, &'a mut VisualTestContext, Vec<BlockId>) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let ids = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let mut ids = vec![page.ensure_block().unwrap()];
        page.set_text(ids[0], texts[0]).unwrap();
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

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn focus(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId, at: usize) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.focus_page_at(Caret { block, at }, window, cx);
        });
    });
    draw(cx);
}

fn texts(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .rows()
            .unwrap()
            .into_iter()
            .map(|row| row.text.unwrap_or_default())
            .collect()
    })
}

fn caret(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext) -> Option<Caret> {
    shell.update(cx, |shell, _| shell.page().unwrap().caret().unwrap())
}

/// Calls the page's text input the way the platform would.
fn ime<R>(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    f: impl FnOnce(&mut Shell, &mut gpui::Window, &mut gpui::Context<Shell>) -> R,
) -> R {
    let result = cx.update(|window, cx| shell.update(cx, |shell, cx| f(shell, window, cx)));
    draw(cx);
    result
}

// ---- Typing and keys ----

#[gpui::test]
fn typing_and_the_structural_keys_go_to_the_editor(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["Hello"]);
    focus(&shell, cx, ids[0], 5);
    cx.simulate_input(" world");
    cx.simulate_keystrokes("left left left left left enter");
    assert_eq!(texts(&shell, cx), vec!["Hello ", "world"]);
    cx.simulate_keystrokes("tab");
    draw(cx);
    let depth = shell.update(cx, |shell, _| shell.page().unwrap().rows().unwrap()[1].depth);
    assert_eq!(depth, 1, "Tab nests the block");
    // Backspace at the start of a paragraph merges it into the block above (the spec's rule).
    cx.simulate_keystrokes("backspace");
    draw(cx);
    assert_eq!(texts(&shell, cx), vec!["Hello world"]);
    assert_eq!(caret(&shell, cx), Some(Caret { block: ids[0], at: 6 }));
}

#[gpui::test]
fn shortcuts_format_and_select(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["make bold"]);
    focus(&shell, cx, ids[0], 9);
    cx.simulate_keystrokes("shift-left shift-left shift-left shift-left secondary-b");
    draw(cx);
    let spans = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().text(ids[0]).unwrap().unwrap()
    });
    assert_eq!(spans.len(), 2);
    assert!(spans[1].marks.bold && spans[1].text == "bold", "{spans:?}");
    // Cmd+B with just a caret formats what is typed next.
    cx.simulate_keystrokes("right secondary-i");
    cx.simulate_input("!");
    let spans = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().text(ids[0]).unwrap().unwrap()
    });
    let last = spans.last().unwrap();
    assert_eq!(last.text, "!");
    assert!(last.marks.italic);
    cx.simulate_keystrokes("secondary-a");
    let selected = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert!(selected.is_some(), "Cmd+A selects the block's text");
}

#[gpui::test]
fn copy_and_paste_carry_formatting_through_the_clipboard(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["one", ""]);
    focus(&shell, cx, ids[0], 3);
    cx.simulate_keystrokes("secondary-a secondary-b secondary-c");
    let item = cx.read_from_clipboard().unwrap();
    assert_eq!(item.text().as_deref(), Some("**one**"), "Markdown for other apps");
    focus(&shell, cx, ids[1], 0);
    cx.simulate_keystrokes("secondary-v");
    draw(cx);
    let spans = shell.update(cx, |shell, _| {
        shell.page().unwrap().doc().text(ids[1]).unwrap().unwrap()
    });
    assert_eq!(
        (spans[0].text.as_str(), spans[0].marks.bold),
        ("one", true),
        "Leaf's own fragment keeps the bold"
    );
    // Text from another app, on a new line: Markdown is read.
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("- from elsewhere".into()));
    cx.simulate_keystrokes("enter secondary-v");
    draw(cx);
    assert_eq!(
        texts(&shell, cx).last().map(String::as_str),
        Some("from elsewhere")
    );
}

/// E17-S05: a heading copied with only the caret in it, or with all its text selected, pastes back as a
/// heading through the clipboard, with its bold.
#[gpui::test]
fn copying_a_block_keeps_its_kind_through_the_clipboard(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["Plans", "", ""]);
    shell.update(cx, |shell, _| {
        let page = shell.page().unwrap();
        page.turn_into(ids[0], leaf_model::BlockKind::Heading { level: 2 })
            .unwrap();
    });
    focus(&shell, cx, ids[0], 2);
    cx.simulate_keystrokes("secondary-a secondary-b");
    // Just the caret: Cmd+C copies the block.
    focus(&shell, cx, ids[0], 2);
    cx.simulate_keystrokes("secondary-c");
    let item = cx.read_from_clipboard().unwrap();
    assert_eq!(
        item.text().as_deref(),
        Some("## **Plans**"),
        "Markdown for other apps"
    );
    focus(&shell, cx, ids[1], 0);
    cx.simulate_keystrokes("secondary-v");
    draw(cx);
    let (kind, spans) = shell.update(cx, |shell, _| {
        let doc = shell.page().unwrap().doc();
        (doc.kind(ids[1]).unwrap(), doc.text(ids[1]).unwrap().unwrap())
    });
    assert_eq!(
        kind,
        leaf_model::BlockKind::Heading { level: 2 },
        "pasted as a heading"
    );
    assert_eq!((spans[0].text.as_str(), spans[0].marks.bold), ("Plans", true));

    // All of its text selected (Cmd+A once selects the block's text): the same.
    focus(&shell, cx, ids[0], 0);
    cx.simulate_keystrokes("secondary-a secondary-c");
    focus(&shell, cx, ids[2], 0);
    cx.simulate_keystrokes("secondary-v");
    draw(cx);
    let kind = shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(ids[2]).unwrap());
    assert_eq!(kind, leaf_model::BlockKind::Heading { level: 2 });
}

// ---- Input methods ----

#[gpui::test]
fn a_japanese_composition_writes_nothing_until_committed(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["ab"]);
    focus(&shell, cx, ids[0], 1);
    // Typing "kanji" with the Japanese IME: each key updates the draft.
    for draft in ["k", "か", "かn", "かん", "かんj", "かんじ"] {
        let len = draft.encode_utf16().count();
        ime(&shell, cx, |s, w, cx| {
            s.replace_and_mark_text_in_range(None, draft, Some(len..len), w, cx);
        });
    }
    assert_eq!(
        texts(&shell, cx),
        vec!["ab"],
        "the page is untouched while composing"
    );
    let marked = ime(&shell, cx, |s, w, cx| s.marked_text_range(w, cx));
    assert_eq!(marked, Some(1..4));
    let mut adjusted = None;
    let seen = ime(&shell, cx, |s, w, cx| {
        s.text_for_range(0..5, &mut adjusted, w, cx)
    });
    assert_eq!(
        seen.as_deref(),
        Some("aかんじb"),
        "the input method sees its draft in place"
    );
    let bounds = ime(&shell, cx, |s, w, cx| {
        s.bounds_for_range(1..4, gpui::Bounds::default(), w, cx)
    });
    assert!(
        bounds.is_some(),
        "the candidate window can be placed under the draft"
    );
    // Choosing 漢字 commits it, as one undo step.
    ime(&shell, cx, |s, w, cx| {
        s.replace_text_in_range(None, "漢字", w, cx);
    });
    assert_eq!(texts(&shell, cx), vec!["a漢字b"]);
    assert_eq!(caret(&shell, cx), Some(Caret { block: ids[0], at: 3 }));
    assert_eq!(ime(&shell, cx, |s, w, cx| s.marked_text_range(w, cx)), None);
    // Undo takes the commit back. (The test's own setup is within the undo merge interval, so it may
    // go further back too; only the commit matters here.)
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    assert!(!texts(&shell, cx).concat().contains('漢'));
}

#[gpui::test]
fn a_hindi_composition_and_reconversion_of_a_selection(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["नमस्ते "]);
    focus(&shell, cx, ids[0], 7);
    for draft in ["क", "कि"] {
        ime(&shell, cx, |s, w, cx| {
            s.replace_and_mark_text_in_range(None, draft, None, w, cx);
        });
    }
    ime(&shell, cx, |s, w, cx| s.replace_text_in_range(None, "कि", w, cx));
    assert_eq!(texts(&shell, cx), vec!["नमस्ते कि"]);
    // Reconversion: the input method composes over existing text (the first word), then commits.
    ime(&shell, cx, |s, w, cx| {
        s.replace_and_mark_text_in_range(Some(0..6), "नमस्कार", None, w, cx);
    });
    assert_eq!(texts(&shell, cx), vec!["नमस्ते कि"], "still a draft");
    ime(&shell, cx, |s, w, cx| {
        s.replace_text_in_range(None, "नमस्कार", w, cx);
    });
    assert_eq!(texts(&shell, cx), vec!["नमस्कार कि"]);
}

#[gpui::test]
fn an_abandoned_composition_keeps_what_was_shown_and_an_empty_one_keeps_the_selection(
    cx: &mut TestAppContext,
) {
    let (shell, cx, ids) = open(cx, &["keep me"]);
    focus(&shell, cx, ids[0], 7);
    ime(&shell, cx, |s, w, cx| {
        s.replace_and_mark_text_in_range(None, "!", None, w, cx);
    });
    ime(&shell, cx, gpui::EntityInputHandler::unmark_text);
    assert_eq!(texts(&shell, cx), vec!["keep me!"]);
    // An empty draft over a selection never deletes it.
    shell.update(cx, |shell, _| shell.page().unwrap().select(ids[0], 0..4).unwrap());
    ime(&shell, cx, |s, w, cx| {
        s.replace_and_mark_text_in_range(None, "x", None, w, cx);
    });
    ime(&shell, cx, |s, w, cx| {
        s.replace_and_mark_text_in_range(None, "", None, w, cx);
    });
    assert_eq!(texts(&shell, cx), vec!["keep me!"]);
    let selected = ime(&shell, cx, |s, w, cx| s.selected_text_range(false, w, cx)).unwrap();
    assert_eq!(selected.range, 0..4);
}

#[gpui::test]
fn the_emoji_picker_and_accent_menu_insert_and_replace(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["cafe"]);
    focus(&shell, cx, ids[0], 4);
    // The accent menu replaces the "e" just typed.
    ime(&shell, cx, |s, w, cx| {
        s.replace_text_in_range(Some(3..4), "é", w, cx);
    });
    // The emoji picker inserts at the caret; a skin-toned emoji is four UTF-16 units.
    ime(&shell, cx, |s, w, cx| s.replace_text_in_range(None, " 👍🏽", w, cx));
    assert_eq!(texts(&shell, cx), vec!["café 👍🏽"]);
    assert_eq!(
        ime(&shell, cx, gpui::EntityInputHandler::text_length_utf16),
        Some(9)
    );
    let selected = ime(&shell, cx, |s, w, cx| s.selected_text_range(false, w, cx)).unwrap();
    assert_eq!(selected.range, 9..9, "the caret after the emoji, in UTF-16");
}

#[test]
fn utf16_and_character_offsets_convert_both_ways() {
    let text = "a👍🏽é";
    assert_eq!(to_utf16(text, &(1..3)), 1..5);
    assert_eq!(from_utf16(text, &(1..5)), 1..3);
    assert_eq!(from_utf16(text, &(0..99)), 0..4, "clamped to the text");
    assert_eq!(
        from_utf16(text, &(2..2)),
        2..2,
        "inside a surrogate pair: the next character"
    );
}

// ---- The mouse ----

/// Where the caret at `at` in `block` is drawn, nudged to the middle of the line.
fn point_of(
    shell: &gpui::Entity<Shell>,
    cx: &mut VisualTestContext,
    block: BlockId,
    at: usize,
) -> gpui::Point<gpui::Pixels> {
    let (top, height) = shell.read_with(cx, |shell, _| {
        shell.blocks.painted.caret_bounds(block, at).unwrap()
    });
    gpui::point(top.x + gpui::px(0.5), top.y + height / 2.0)
}

fn mouse_down(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, clicks: usize, shift: bool) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers {
            shift,
            ..Modifiers::none()
        },
        click_count: clicks,
        first_mouse: false,
    });
}

#[gpui::test]
fn clicking_places_the_caret_and_dragging_selects(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["first line", "second line"]);
    let at = point_of(&shell, cx, ids[0], 3);
    mouse_down(cx, at, 1, false);
    cx.simulate_event(MouseUpEvent {
        button: MouseButton::Left,
        position: at,
        modifiers: Modifiers::none(),
        click_count: 1,
    });
    draw(cx);
    assert_eq!(caret(&shell, cx), Some(Caret { block: ids[0], at: 3 }));

    // Drag from "first|" into the second block.
    let from = point_of(&shell, cx, ids[0], 5);
    let to = point_of(&shell, cx, ids[1], 6);
    mouse_down(cx, from, 1, false);
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
    draw(cx);
    let selection = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert_eq!(
        selection,
        Some(leaf_editor::Selection::Text {
            start: Caret { block: ids[0], at: 5 },
            end: Caret { block: ids[1], at: 6 }
        })
    );
    // Shift-click extends it.
    let end = point_of(&shell, cx, ids[1], 11);
    mouse_down(cx, end, 1, true);
    draw(cx);
    let selection = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert!(matches!(selection, Some(leaf_editor::Selection::Text { end, .. }) if end.at == 11));
}

#[gpui::test]
fn double_click_selects_a_word_and_triple_click_the_block(cx: &mut TestAppContext) {
    let (shell, cx, ids) = open(cx, &["pick this word"]);
    let at = point_of(&shell, cx, ids[0], 7);
    mouse_down(cx, at, 2, false);
    draw(cx);
    let word = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert_eq!(
        word,
        Some(leaf_editor::Selection::Text {
            start: Caret { block: ids[0], at: 5 },
            end: Caret { block: ids[0], at: 9 }
        })
    );
    mouse_down(cx, at, 3, false);
    draw(cx);
    let block = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert!(
        matches!(block, Some(leaf_editor::Selection::Text { start, end }) if start.at == 0 && end.at == 14)
    );
}
