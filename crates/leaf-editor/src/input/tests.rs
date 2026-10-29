//! Tests for typing and input-method composition (E04-S04).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, History, PageCommand, PageDoc, PageOutcome};

use super::{Preedit, TextRange};
use crate::{Motion, PageEditor};

fn page(blocks: &[(BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let ids = blocks
        .iter()
        .map(|(kind, text)| {
            let index = doc.root_blocks().len();
            let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
                parent: None,
                index,
                kind: kind.clone(),
                text: (*text).into(),
            }) else {
                panic!("insert failed");
            };
            id
        })
        .collect();
    // Loaded from bytes, as when a page is opened: its setup is saved and can't be undone, so
    // the tests see exactly what the editor wrote.
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn para(text: &str) -> (BlockKind, &str) {
    (BlockKind::Paragraph, text)
}

fn text(editor: &PageEditor, id: BlockId) -> String {
    editor.doc().plain_text(id).unwrap().unwrap_or_default()
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

fn range(block: BlockId, range: std::ops::Range<usize>) -> TextRange {
    TextRange { block, range }
}

#[test]
fn typing_inserts_at_the_caret_and_moves_it_along() {
    let (mut editor, ids) = page(&[para("ac")]);
    assert_eq!(
        editor.insert_text("x").unwrap(),
        None,
        "no caret, nowhere to type"
    );
    editor.set_caret(at(ids[0], 1)).unwrap();
    assert_eq!(editor.insert_text("b").unwrap(), Some(at(ids[0], 2)));
    assert_eq!(editor.insert_text("👋🏽").unwrap(), Some(at(ids[0], 4)));
    assert_eq!(text(&editor, ids[0]), "ab👋🏽c");
    assert_eq!(editor.insert_text("").unwrap(), Some(at(ids[0], 4)));
    assert_eq!(text(&editor, ids[0]), "ab👋🏽c");
}

#[test]
fn typing_replaces_the_selection_in_one_undo_step() {
    let (mut editor, ids) = page(&[para("hello world")]);
    editor.select(ids[0], 6..11).unwrap();
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 6..11)));
    assert_eq!(editor.insert_text("there").unwrap(), Some(at(ids[0], 11)));
    assert_eq!(text(&editor, ids[0]), "hello there");
    assert_eq!(
        editor.selection().unwrap(),
        None,
        "typing collapses the selection"
    );

    assert_eq!(
        editor.doc.undo().unwrap(),
        History::Done {
            caret: Some(at(ids[0], 11))
        },
        "the caret as it was before typing: the selection's end"
    );
    assert_eq!(text(&editor, ids[0]), "hello world");
}

#[test]
fn a_selection_snaps_to_whole_characters_and_follows_edits() {
    let family = "👨\u{200d}👩\u{200d}👧"; // 5 scalars
    let (mut editor, ids) = page(&[para(&format!("a{family}b"))]);
    editor.select(ids[0], 2..7).unwrap();
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 1..7)));
    editor
        .doc
        .apply(PageCommand::InsertText {
            id: ids[0],
            pos: 0,
            text: ">>".into(),
        })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 3..9)));
}

#[test]
fn moving_the_caret_drops_the_selection_and_an_empty_one_is_none() {
    let (mut editor, ids) = page(&[para("hello")]);
    editor.select(ids[0], 1..4).unwrap();
    editor.move_caret(Motion::Left).unwrap();
    assert_eq!(editor.selection().unwrap(), None);
    editor.select(ids[0], 2..2).unwrap();
    assert_eq!(editor.selection().unwrap(), None);
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 2)));
}

#[test]
fn a_selection_backwards_reads_forwards() {
    let (mut editor, ids) = page(&[para("hello")]);
    // Dragged right to left: the caret ends up at the start.
    editor
        .select(ids[0], std::ops::Range { start: 4, end: 1 })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 1..4)));
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 1)));
}

#[test]
fn a_selection_whose_other_end_vanished_is_no_selection() {
    let (mut editor, ids) = page(&[para("hello")]);
    editor.select(ids[0], 1..4).unwrap();
    editor
        .doc
        .apply(PageCommand::DeleteText {
            id: ids[0],
            range: 0..5,
        })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), None);
}

#[test]
fn a_composition_writes_nothing_until_committed() {
    let (mut editor, ids) = page(&[para("ab")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.set_preedit("k", 1);
    editor.set_preedit("कि", 2);
    assert_eq!(
        editor.preedit().unwrap(),
        Some(Preedit {
            at: range(ids[0], 1..1),
            text: "कि".into(),
            cursor: 2,
        })
    );
    assert_eq!(text(&editor, ids[0]), "ab");
    assert!(!editor.doc().has_unsaved_changes(), "nothing to save");
    assert_eq!(editor.doc.undo().unwrap(), History::Nothing, "no undo step");

    assert_eq!(editor.commit_preedit("कि").unwrap(), Some(at(ids[0], 3)));
    assert_eq!(editor.preedit().unwrap(), None);
    assert_eq!(text(&editor, ids[0]), "aकिb");
    editor.doc.undo().unwrap();
    assert_eq!(text(&editor, ids[0]), "ab", "the commit is one step");
}

#[test]
fn a_composition_over_a_selection_covers_it_and_the_commit_replaces_it() {
    let (mut editor, ids) = page(&[para("say hello")]);
    editor.select(ids[0], 4..9).unwrap();
    editor.set_preedit("こんにちは", 5);
    assert_eq!(editor.preedit().unwrap().map(|p| p.at), Some(range(ids[0], 4..9)));
    assert_eq!(text(&editor, ids[0]), "say hello", "the selection is still there");
    editor.commit_preedit("こんにちは").unwrap();
    assert_eq!(text(&editor, ids[0]), "say こんにちは");
}

#[test]
fn an_empty_composition_never_deletes_the_selection() {
    let (mut editor, ids) = page(&[para("keep me")]);
    editor.select(ids[0], 0..4).unwrap();
    editor.set_preedit("x", 1);
    editor.set_preedit("", 0);
    assert_eq!(editor.preedit().unwrap(), None);
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 0..4)));

    editor.set_preedit("x", 1);
    editor.commit_preedit("").unwrap();
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 0..4)));

    editor.set_preedit("x", 1);
    editor.cancel_preedit();
    assert_eq!(editor.preedit().unwrap(), None);
    assert_eq!(editor.selection().unwrap(), Some(range(ids[0], 0..4)));
    assert_eq!(text(&editor, ids[0]), "keep me");
    assert!(!editor.doc().has_unsaved_changes());
}

#[test]
fn the_draft_cursor_snaps_and_the_draft_needs_a_text_block() {
    let (mut editor, ids) = page(&[para("a"), (BlockKind::Divider, "")]);
    editor.set_preedit("x", 0);
    assert_eq!(editor.preedit().unwrap(), None, "no caret yet");
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.set_preedit("👍🏽", 1);
    assert_eq!(editor.preedit().unwrap().map(|p| p.cursor), Some(0));
    editor.set_caret(at(ids[1], 0)).unwrap();
    assert_eq!(editor.preedit().unwrap(), None);
    assert_eq!(editor.commit_preedit("👍🏽").unwrap(), Some(at(ids[1], 0)));
    assert_eq!(editor.doc().plain_text(ids[1]).unwrap(), None);
}

#[test]
fn the_draft_follows_the_caret_when_other_edits_land() {
    let (mut editor, ids) = page(&[para("ab")]);
    editor.set_caret(at(ids[0], 2)).unwrap();
    editor.set_preedit("x", 1);
    let mut theirs = PageDoc::from_bytes(&editor.doc().to_bytes().unwrap()).unwrap();
    theirs
        .apply(PageCommand::InsertText {
            id: ids[0],
            pos: 0,
            text: "12".into(),
        })
        .unwrap();
    editor
        .doc
        .import_changes(&theirs.unsaved_changes().unwrap().unwrap())
        .unwrap();
    assert_eq!(editor.preedit().unwrap().map(|p| p.at), Some(range(ids[0], 4..4)));
    editor.commit_preedit("x").unwrap();
    assert_eq!(text(&editor, ids[0]), "12abx");
}
