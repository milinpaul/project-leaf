//! E21-S01: editing rules for callouts, one test per "Editor behaviour" row.

use leaf_model::{BlockKind, CalloutTone};

use super::{at, outline, page, press, press_and_undo, row};
use crate::{Key, KeyOutcome};

fn callout(text: &str) -> (usize, BlockKind, &str) {
    (
        0,
        BlockKind::Callout {
            tone: CalloutTone::Warning,
        },
        text,
    )
}

#[test]
fn row_enter_in_a_callout_adds_a_line_inside_it() {
    let (mut editor, ids) = page(&[callout("Pack light")]);
    let block = ids[0];
    assert_eq!(
        press_and_undo(&mut editor, at(block, 4), Key::Enter),
        KeyOutcome::Caret(at(block, 5))
    );
    press(&mut editor, Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![row(0, callout("").1, "Pack\n\n light")],
        "line breaks, never new blocks; an empty line in the middle doesn't leave"
    );
    assert_eq!(editor.doc().len(), 1);
}

#[test]
fn row_enter_on_an_empty_last_line_leaves_the_callout() {
    let (mut editor, ids) = page(&[callout("Bring a jacket\n")]);
    let KeyOutcome::Caret(out) = press_and_undo(&mut editor, at(ids[0], 15), Key::Enter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, callout("").1, "Bring a jacket"),
            row(0, BlockKind::Paragraph, ""),
        ],
        "the empty line goes, and a paragraph follows"
    );
    assert_eq!(out.at, 0);
    assert_ne!(out.block, ids[0]);
}

#[test]
fn enter_in_an_empty_callout_leaves_it_and_mod_enter_leaves_from_anywhere() {
    let (mut editor, ids) = page(&[callout(""), callout("one\ntwo")]);
    press_and_undo(&mut editor, at(ids[0], 0), Key::Enter);
    assert_eq!(outline(&editor)[1], row(0, BlockKind::Paragraph, ""));

    let (mut editor, ids) = page(&[callout("one\ntwo")]);
    press_and_undo(&mut editor, at(ids[0], 2), Key::ModEnter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, callout("").1, "one\ntwo"),
            row(0, BlockKind::Paragraph, "")
        ]
    );
}

#[test]
fn row_backspace_at_the_start_of_a_callout_makes_it_a_paragraph() {
    let (mut editor, ids) = page(&[(0, BlockKind::Paragraph, "above"), callout("Note this")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 0), Key::Backspace),
        KeyOutcome::Caret(at(ids[1], 0))
    );
    assert_eq!(outline(&editor)[1], row(0, BlockKind::Paragraph, "Note this"));
}

#[test]
fn enter_over_a_selection_in_a_callout_replaces_it_with_a_line_break() {
    let (mut editor, ids) = page(&[callout("a;b;")]);
    editor.select(ids[0], 2..3).unwrap();
    assert_eq!(press(&mut editor, Key::Enter), KeyOutcome::Caret(at(ids[0], 3)));
    assert_eq!(outline(&editor)[0].2, "a;\n;");
}
