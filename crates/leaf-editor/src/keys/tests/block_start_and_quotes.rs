//! E17-S13: Enter at the very start of a block keeps the block as it is, and a quote holds lines
//! like a callout, one test per "Editor behaviour" row.

use leaf_model::BlockKind;

use super::{at, outline, page, press_and_undo, row};
use crate::{Key, KeyOutcome};

fn heading(level: u8, text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Heading { level }, text)
}

fn quote(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Quote, text)
}

#[test]
fn row_enter_at_the_start_of_a_heading_adds_a_paragraph_above_and_keeps_the_heading() {
    for level in 1..=3 {
        let (mut editor, ids) = page(&[heading(level, "Packing list")]);
        assert_eq!(
            press_and_undo(&mut editor, at(ids[0], 0), Key::Enter),
            KeyOutcome::Caret(at(ids[0], 0)),
            "the caret stays at the start of the same heading"
        );
        assert_eq!(
            outline(&editor),
            vec![
                row(0, BlockKind::Paragraph, ""),
                row(0, BlockKind::Heading { level }, "Packing list"),
            ],
            "level {level}"
        );
    }
}

#[test]
fn enter_at_the_start_of_a_paragraph_or_nested_block_adds_the_empty_line_at_its_level() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "trip"),
        (1, BlockKind::Heading { level: 2 }, "Day one"),
    ]);
    press_and_undo(&mut editor, at(ids[1], 0), Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, "trip"),
            row(1, BlockKind::Paragraph, ""),
            row(1, BlockKind::Heading { level: 2 }, "Day one"),
        ]
    );
    let (mut editor, ids) = page(&[(0, BlockKind::Paragraph, "words")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 0), Key::Enter),
        KeyOutcome::Caret(at(ids[0], 0))
    );
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, ""),
            row(0, BlockKind::Paragraph, "words")
        ]
    );
}

#[test]
fn enter_mid_heading_still_splits_into_a_paragraph() {
    let (mut editor, ids) = page(&[heading(1, "Packing list")]);
    press_and_undo(&mut editor, at(ids[0], 7), Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Heading { level: 1 }, "Packing"),
            row(0, BlockKind::Paragraph, " list"),
        ]
    );
}

#[test]
fn enter_at_the_start_of_a_list_item_still_keeps_the_list_going() {
    let (mut editor, ids) = page(&[(0, BlockKind::BulletedList, "eggs")]);
    press_and_undo(&mut editor, at(ids[0], 0), Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, ""),
            row(0, BlockKind::BulletedList, "eggs"),
        ]
    );
}

#[test]
fn row_enter_in_a_quote_adds_a_line_inside_it() {
    let (mut editor, ids) = page(&[quote("To be or not")]);
    let block = ids[0];
    assert_eq!(
        press_and_undo(&mut editor, at(block, 5), Key::Enter),
        KeyOutcome::Caret(at(block, 6))
    );
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Quote, "To be\n or not")]);
    assert_eq!(editor.doc().len(), 1, "a line break, never a new block");

    let (mut editor, ids) = page(&[quote("said the poet")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 0), Key::Enter),
        KeyOutcome::Caret(at(ids[0], 1)),
        "at the start too: the quote keeps its text and its style"
    );
    assert_eq!(
        outline(&editor),
        vec![row(0, BlockKind::Quote, "\nsaid the poet")]
    );
}

#[test]
fn row_enter_on_an_empty_last_line_leaves_the_quote() {
    let (mut editor, ids) = page(&[quote("line one\n")]);
    let KeyOutcome::Caret(out) = press_and_undo(&mut editor, at(ids[0], 9), Key::Enter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Quote, "line one"),
            row(0, BlockKind::Paragraph, "")
        ],
        "the empty line goes, and a paragraph follows"
    );
    assert_eq!(out.at, 0);
    assert_ne!(out.block, ids[0]);
}

#[test]
fn enter_in_an_empty_quote_leaves_it_and_mod_enter_leaves_from_anywhere() {
    let (mut editor, ids) = page(&[quote("")]);
    press_and_undo(&mut editor, at(ids[0], 0), Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![row(0, BlockKind::Quote, ""), row(0, BlockKind::Paragraph, "")]
    );

    let (mut editor, ids) = page(&[quote("one\ntwo")]);
    press_and_undo(&mut editor, at(ids[0], 2), Key::ModEnter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Quote, "one\ntwo"),
            row(0, BlockKind::Paragraph, "")
        ]
    );
}
