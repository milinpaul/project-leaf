//! Tests for undo and redo with the selection (E04-S08): undo restores exactly what was selected
//! before an edit, redo what the edit left, across text and structural edits.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, MarkKind, PageCommand, PageDoc, PageOutcome};

use crate::{Key, Motion, PageEditor, Selection};

fn page(blocks: &[(usize, BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let mut ids: Vec<BlockId> = Vec::new();
    let mut parents: Vec<BlockId> = Vec::new();
    for (depth, kind, text) in blocks {
        parents.truncate(*depth);
        let parent = parents.last().copied();
        let index = match parent {
            Some(p) => doc.children(p).unwrap().len(),
            None => doc.root_blocks().len(),
        };
        let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
            parent,
            index,
            kind: kind.clone(),
            text: (*text).into(),
        }) else {
            panic!("insert failed");
        };
        parents.push(id);
        ids.push(id);
    }
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn p(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Paragraph, text)
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

fn text(start: Caret, end: Caret) -> Selection {
    Selection::Text { start, end }
}

fn texts(editor: &PageEditor) -> Vec<String> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| row.text.unwrap_or_default())
        .collect()
}

/// Undoes, checks the selection, redoes, checks the selection after.
fn round_trip(
    editor: &mut PageEditor,
    before: Option<&Selection>,
    caret_before: Caret,
    after: Option<&Selection>,
    caret_after: Caret,
) {
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.selection().unwrap().as_ref(),
        before,
        "undo: the selection before"
    );
    assert_eq!(editor.caret().unwrap(), Some(caret_before), "undo: the caret end");
    assert!(editor.redo().unwrap());
    assert_eq!(
        editor.selection().unwrap().as_ref(),
        after,
        "redo: the selection after"
    );
    assert_eq!(editor.caret().unwrap(), Some(caret_after), "redo: the caret");
}

#[test]
fn typing_over_a_selection_in_a_block() {
    let (mut editor, ids) = page(&[p("hello world")]);
    let b = ids[0];
    // Selected right to left: the caret is at the start, and comes back there.
    editor.select_between(at(b, 11), at(b, 6)).unwrap();
    editor.insert_text("there").unwrap();
    // The user moves away before undoing; redo still goes to where the typing ended.
    editor.set_caret(at(b, 0)).unwrap();
    round_trip(
        &mut editor,
        Some(&text(at(b, 6), at(b, 11))),
        at(b, 6),
        None,
        at(b, 11),
    );
}

#[test]
fn typing_over_a_selection_across_blocks() {
    let (mut editor, ids) = page(&[p("one"), p("two"), p("three")]);
    editor.select_between(at(ids[0], 1), at(ids[2], 2)).unwrap();
    editor.insert_text("X").unwrap();
    assert_eq!(texts(&editor), vec!["oXree"]);
    round_trip(
        &mut editor,
        Some(&text(at(ids[0], 1), at(ids[2], 2))),
        at(ids[2], 2),
        None,
        at(ids[0], 2),
    );
    assert_eq!(texts(&editor), vec!["oXree"]);
}

#[test]
fn structural_keys_enter_backspace_and_tab() {
    let (mut editor, ids) = page(&[p("ab"), p("cd")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.press(Key::Enter).unwrap();
    let new = editor.rows().unwrap()[1].id;
    round_trip(&mut editor, None, at(ids[0], 1), None, at(new, 0));

    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.press(Key::Backspace).unwrap();
    round_trip(&mut editor, None, at(ids[1], 0), None, at(new, 1));

    // "cd" was merged into the new block: the page is "a", "bcd".
    editor.select_between(at(new, 2), at(ids[0], 0)).unwrap();
    editor.press(Key::Tab).unwrap();
    let kept = text(at(ids[0], 0), at(new, 2));
    round_trip(
        &mut editor,
        Some(&kept),
        at(ids[0], 0),
        Some(&kept),
        at(ids[0], 0),
    );
}

#[test]
fn deleting_selected_blocks_brings_the_block_selection_back() {
    let (mut editor, ids) = page(&[p("a"), p("b"), (1, BlockKind::Paragraph, "child"), p("c")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.select_blocks(&[ids[1]]).unwrap();
    editor.press(Key::Delete).unwrap();
    assert_eq!(texts(&editor), vec!["a", "c"]);
    round_trip(
        &mut editor,
        Some(&Selection::Blocks(vec![ids[1]])),
        at(ids[0], 1),
        None,
        at(ids[0], 1),
    );
}

#[test]
fn formatting_keeps_the_selection_both_ways() {
    let (mut editor, ids) = page(&[p("bold me")]);
    let b = ids[0];
    editor.select(b, 0..4).unwrap();
    editor.toggle_mark(MarkKind::Bold).unwrap();
    let selected = text(at(b, 0), at(b, 4));
    round_trip(&mut editor, Some(&selected), at(b, 4), Some(&selected), at(b, 4));
}

#[test]
fn several_steps_undo_and_redo_in_order() {
    let (mut editor, ids) = page(&[p("")]);
    let b = ids[0];
    editor.set_caret(at(b, 0)).unwrap();
    editor.insert_text("ab").unwrap();
    editor.select(b, 0..2).unwrap();
    editor.insert_text("XY").unwrap();
    editor.press(Key::Enter).unwrap();

    assert!(editor.undo().unwrap());
    assert_eq!(editor.caret().unwrap(), Some(at(b, 2)), "before Enter");
    assert!(editor.undo().unwrap());
    assert_eq!(
        editor.selection().unwrap(),
        Some(text(at(b, 0), at(b, 2))),
        "before typing over"
    );
    assert!(editor.undo().unwrap());
    assert_eq!(editor.caret().unwrap(), Some(at(b, 0)), "before the first typing");
    assert!(!editor.undo().unwrap(), "nothing left");

    assert!(editor.redo().unwrap());
    assert_eq!(editor.caret().unwrap(), Some(at(b, 2)));
    assert!(editor.redo().unwrap());
    assert_eq!(editor.caret().unwrap(), Some(at(b, 2)), "after typing XY");
    assert_eq!(editor.selection().unwrap(), None);
    assert!(editor.redo().unwrap());
    assert_eq!(
        editor.caret().unwrap().map(|c| c.at),
        Some(0),
        "at the start of the new block"
    );
    assert!(!editor.redo().unwrap());
}

#[test]
fn a_key_that_changes_nothing_does_not_disturb_redo() {
    let (mut editor, ids) = page(&[p("abc")]);
    let b = ids[0];
    editor.set_caret(at(b, 3)).unwrap();
    editor.insert_text("d").unwrap();
    editor.move_caret(Motion::LineStart).unwrap();
    // Shift+Tab at the top level changes nothing.
    editor.press(Key::ShiftTab).unwrap();
    editor.undo().unwrap();
    editor.redo().unwrap();
    assert_eq!(editor.caret().unwrap(), Some(at(b, 4)));
}

#[test]
fn undo_without_a_recorded_selection_or_after_a_composition() {
    let (mut editor, ids) = page(&[p("ab")]);
    // An edit made straight on the document, outside the editor's actions: nothing recorded.
    editor
        .doc
        .apply(PageCommand::InsertText {
            id: ids[0],
            pos: 0,
            text: "x".into(),
        })
        .unwrap();
    editor.set_preedit("k", 1);
    assert!(editor.undo().unwrap());
    assert_eq!(editor.caret().unwrap(), None, "no caret to restore");
    assert_eq!(editor.preedit().unwrap(), None, "undo ends a composition");
    assert_eq!(texts(&editor), vec!["ab"]);
}
