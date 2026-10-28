//! Tests for the page editor's caret: moving with the keyboard and following changes (E04-S02).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use super::Motion::{self, Left, LineEnd, LineStart, Right, WordLeft, WordRight};
use crate::PageEditor;

fn page(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    doc.set_undo_merge_interval(0);
    let ids = blocks
        .iter()
        .map(|(kind, text)| add(&mut doc, kind.clone(), text))
        .collect();
    (PageEditor::new(doc), ids)
}

fn add(doc: &mut PageDoc, kind: BlockKind, text: &str) -> BlockId {
    let index = doc.root_blocks().len();
    match doc.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind,
        text: text.into(),
    }) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("insert failed: {other:?}"),
    }
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// Where the caret goes after each motion, in turn.
fn moves(editor: &mut PageEditor, motions: &[Motion]) -> Vec<Caret> {
    motions
        .iter()
        .map(|&m| editor.move_caret(m).unwrap().unwrap())
        .collect()
}

#[test]
fn nothing_moves_before_the_caret_is_placed() {
    let (mut editor, _) = page(&[(BlockKind::Paragraph, "abc")]);
    assert_eq!(editor.caret().unwrap(), None);
    assert_eq!(editor.move_caret(Right).unwrap(), None);
}

#[test]
fn placing_the_caret_inside_an_emoji_snaps_to_its_start() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "a👨\u{200d}👩\u{200d}👧b")]);
    assert_eq!(editor.set_caret(at(ids[0], 3)).unwrap(), at(ids[0], 1));
    assert_eq!(
        moves(&mut editor, &[Right, Right, Left, Left]),
        vec![at(ids[0], 6), at(ids[0], 7), at(ids[0], 6), at(ids[0], 1)]
    );
}

#[test]
fn arrows_cross_into_neighbouring_text_blocks_and_skip_dividers() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "ab"),
        (BlockKind::Divider, ""),
        (BlockKind::Heading { level: 1 }, "न"),
    ]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    assert_eq!(
        moves(&mut editor, &[Right, Right, Right, Right, Right]),
        vec![
            at(ids[0], 2),
            at(ids[2], 0),
            at(ids[2], 1),
            at(ids[2], 1),
            at(ids[2], 1)
        ],
        "the end of the page stays put"
    );
    assert_eq!(
        moves(&mut editor, &[Left, Left, Left, Left, Left]),
        vec![
            at(ids[2], 0),
            at(ids[0], 2),
            at(ids[0], 1),
            at(ids[0], 0),
            at(ids[0], 0)
        ]
    );
}

#[test]
fn word_jumps_stay_in_the_block_until_its_edge_then_cross() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "one two"),
        (BlockKind::BulletedList, "  three"),
    ]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    assert_eq!(
        moves(&mut editor, &[WordRight, WordRight, WordRight, WordRight]),
        vec![at(ids[0], 3), at(ids[0], 7), at(ids[1], 0), at(ids[1], 7)]
    );
    assert_eq!(
        moves(&mut editor, &[WordLeft, WordLeft, WordLeft, WordLeft]),
        vec![at(ids[1], 2), at(ids[1], 0), at(ids[0], 7), at(ids[0], 4)]
    );
}

#[test]
fn home_and_end_stay_in_the_block_and_follow_code_lines() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "first"),
        (
            BlockKind::Code {
                language: String::new(),
            },
            "let a = 1;\nlet b = 2;",
        ),
    ]);
    editor.set_caret(at(ids[1], 14)).unwrap();
    assert_eq!(
        moves(&mut editor, &[LineStart, LineStart, LineEnd, LineEnd]),
        vec![at(ids[1], 11), at(ids[1], 11), at(ids[1], 21), at(ids[1], 21)]
    );
    editor.set_caret(at(ids[1], 3)).unwrap();
    assert_eq!(moves(&mut editor, &[LineEnd]), vec![at(ids[1], 10)]);
}

#[test]
fn a_caret_on_a_divider_moves_off_it() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "a"),
        (BlockKind::Divider, ""),
        (BlockKind::Paragraph, "b"),
    ]);
    editor.set_caret(at(ids[1], 5)).unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[1], 0)));
    assert_eq!(moves(&mut editor, &[Right]), vec![at(ids[2], 0)]);
    editor.set_caret(at(ids[1], 0)).unwrap();
    assert_eq!(moves(&mut editor, &[Left]), vec![at(ids[0], 1)]);
    editor.set_caret(at(ids[1], 0)).unwrap();
    assert_eq!(moves(&mut editor, &[LineEnd]), vec![at(ids[1], 0)]);
}

#[test]
fn the_caret_follows_typing_before_it_and_undo() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "hello world")]);
    editor.set_caret(at(ids[0], 6)).unwrap();
    editor.set_text(ids[0], "oh, hello world").unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 10)));
    editor.doc.undo().unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 6)));
    editor.doc.redo().unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 10)));
}

#[test]
fn the_caret_comes_back_with_a_block_that_undo_restores_and_keeps_following_it() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "above"), (BlockKind::Paragraph, "hello")]);
    editor.set_caret(at(ids[1], 3)).unwrap();
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[1]] })
        .unwrap();
    editor.doc.undo().unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[1], 3)));
    editor.set_text(ids[1], ">hello").unwrap();
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[1], 4)),
        "re-anchored to the re-created block"
    );
}

#[test]
fn another_peers_edits_move_the_caret() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "नमस्ते world")]);
    let mut theirs = PageDoc::from_bytes(&editor.doc().to_bytes().unwrap()).unwrap();
    editor.set_caret(at(ids[0], 7)).unwrap();
    theirs
        .apply(PageCommand::InsertText {
            id: ids[0],
            pos: 0,
            text: "👋 ".into(),
        })
        .unwrap();
    theirs
        .apply(PageCommand::DeleteText {
            id: ids[0],
            range: 9..14,
        })
        .unwrap();
    let update = theirs.unsaved_changes().unwrap().unwrap();
    editor.doc.import_changes(&update).unwrap();
    assert_eq!(editor.doc().plain_text(ids[0]).unwrap().unwrap(), "👋 नमस्ते ");
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 9)));
}

#[test]
fn when_the_caret_block_is_deleted_the_caret_goes_to_the_nearest_text_above() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "top"),
        (BlockKind::Divider, ""),
        (BlockKind::Paragraph, "gone"),
        (BlockKind::Paragraph, "below"),
    ]);
    editor.set_caret(at(ids[2], 2)).unwrap();
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[2]] })
        .unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 3)));

    editor.set_caret(at(ids[0], 1)).unwrap();
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[0]] })
        .unwrap();
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[3], 0)),
        "nothing with text above: the first text block below"
    );

    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[3]] })
        .unwrap();
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[1], 0)),
        "only a divider left"
    );
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[1]] })
        .unwrap();
    assert_eq!(editor.caret().unwrap(), None, "an empty page");
    assert_eq!(editor.move_caret(Left).unwrap(), None);
}
