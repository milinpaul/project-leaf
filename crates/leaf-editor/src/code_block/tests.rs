//! Tests for code blocks (E07-S09): the language list and Tab inside code.

use leaf_model::BlockKind;

use super::{CODE_LANGUAGES, find_languages, language_label};
use crate::block_actions::tests::{at, p, page};
use crate::{Key, PageEditor, Selection};

fn code(text: &str) -> (usize, BlockKind, &str) {
    (
        0,
        BlockKind::Code {
            language: String::new(),
        },
        text,
    )
}

fn text(editor: &PageEditor, block: leaf_core::BlockId) -> String {
    editor.doc().plain_text(block).unwrap().unwrap()
}

#[test]
fn tab_inserts_spaces_at_the_caret_and_shift_tab_takes_them_off_the_line() {
    let (mut editor, ids) = page(&[p(0, "above"), code("fn main() {\nx\n}")]);
    let block = ids[1];
    editor.set_caret(at(block, 12)).unwrap();
    editor.press(Key::Tab).unwrap();
    assert_eq!(text(&editor, block), "fn main() {\n    x\n}");
    assert_eq!(editor.caret().unwrap(), Some(at(block, 16)));
    assert_eq!(editor.doc().parent(block).unwrap(), None, "not nested");
    editor.press(Key::Tab).unwrap();
    assert_eq!(text(&editor, block), "fn main() {\n        x\n}");

    editor.set_caret(at(block, 21)).unwrap();
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(text(&editor, block), "fn main() {\n    x\n}");
    assert_eq!(editor.caret().unwrap(), Some(at(block, 17)), "after the x still");
    editor.press(Key::ShiftTab).unwrap();
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(
        text(&editor, block),
        "fn main() {\nx\n}",
        "nothing more to take off"
    );
    assert_eq!(editor.caret().unwrap(), Some(at(block, 13)));

    assert!(editor.undo().unwrap());
    assert_eq!(
        text(&editor, block),
        "fn main() {\n    x\n}",
        "each press is one undo step"
    );
}

#[test]
fn a_selection_indents_and_outdents_every_line_it_touches() {
    let (mut editor, ids) = page(&[code("a\nb\nc\nd")]);
    let block = ids[0];
    // From inside "b" to the very start of "d": "d" isn't touched.
    editor.select_between(at(block, 3), at(block, 6)).unwrap();
    editor.press(Key::Tab).unwrap();
    assert_eq!(text(&editor, block), "a\n    b\n    c\nd");
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Text {
            start: at(block, 7),
            end: at(block, 14),
        }),
        "still selected, moved with the text"
    );
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(text(&editor, block), "a\nb\nc\nd");
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Text {
            start: at(block, 3),
            end: at(block, 6),
        })
    );
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(text(&editor, block), "a\nb\nc\nd", "no spaces: nothing happens");
}

#[test]
fn outside_code_or_across_blocks_tab_still_nests() {
    let (mut editor, ids) = page(&[code("x"), p(0, "a"), p(0, "b")]);
    editor.set_caret(at(ids[2], 0)).unwrap();
    editor.press(Key::Tab).unwrap();
    assert_eq!(
        editor.doc().parent(ids[2]).unwrap(),
        Some(ids[1]),
        "a paragraph nests as before"
    );
    let (mut editor, ids) = page(&[p(0, "a"), code("x")]);
    editor.select_between(at(ids[0], 0), at(ids[1], 1)).unwrap();
    editor.press(Key::Tab).unwrap();
    assert_eq!(
        text(&editor, ids[1]),
        "x",
        "a selection across blocks nests them as usual"
    );
}

#[test]
fn the_language_is_set_as_one_step_and_only_on_code() {
    let (mut editor, ids) = page(&[code("x"), p(0, "para")]);
    editor.set_code_language(ids[0], "rust").unwrap();
    assert_eq!(
        editor.doc().kind(ids[0]).unwrap(),
        BlockKind::Code {
            language: "rust".into()
        }
    );
    editor.set_code_language(ids[0], "rust").unwrap();
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.doc().kind(ids[0]).unwrap(),
        BlockKind::Code {
            language: String::new()
        },
        "setting the same language again added no step"
    );
    editor.set_code_language(ids[1], "rust").unwrap();
    assert_eq!(editor.doc().kind(ids[1]).unwrap(), BlockKind::Paragraph);
}

#[test]
fn languages_are_listed_found_and_named() {
    assert_eq!(CODE_LANGUAGES[0].label, "Plain text");
    assert_eq!(find_languages("").len(), CODE_LANGUAGES.len());
    assert_eq!(find_languages("rust")[0].id, "rust");
    assert_eq!(find_languages("ts")[0].id, "typescript");
    assert_eq!(find_languages("py")[0].id, "python");
    assert_eq!(find_languages("c++")[0].id, "cpp");
    assert_eq!(find_languages("zzz"), vec![]);
    assert_eq!(language_label("rust"), "Rust");
    assert_eq!(language_label("Rust"), "Rust");
    assert_eq!(language_label(""), "Plain text");
    assert_eq!(
        language_label("elixir"),
        "elixir",
        "unknown names are shown as stored"
    );
}
