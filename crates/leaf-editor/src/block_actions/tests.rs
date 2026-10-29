//! Tests for block actions (E05-S03): Turn into, Duplicate and Delete on one block or a selection,
//! each one undo step.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, Mark, PageCommand, PageDoc, PageOutcome};

use crate::{BlockAction, PageEditor, Selection};

pub(crate) fn page(blocks: &[(usize, BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
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

pub(crate) fn p(depth: usize, text: &str) -> (usize, BlockKind, &str) {
    (depth, BlockKind::Paragraph, text)
}

pub(crate) fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

pub(crate) fn outline(editor: &PageEditor) -> Vec<(usize, BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.depth,
                editor.doc().kind(row.id).unwrap(),
                row.text.unwrap_or_default(),
            )
        })
        .collect()
}

pub(crate) fn row(depth: usize, kind: BlockKind, text: &str) -> (usize, BlockKind, String) {
    (depth, kind, text.into())
}

/// Runs the action, checks one undo restores the page exactly and redo replays it.
fn act_and_undo(editor: &mut PageEditor, block: BlockId, action: BlockAction) -> Option<Caret> {
    let before = outline(editor);
    let caret = editor.block_action(block, action).unwrap();
    let after = outline(editor);
    assert!(editor.undo().unwrap());
    assert_eq!(outline(editor), before, "one undo step");
    assert!(editor.redo().unwrap());
    assert_eq!(outline(editor), after, "redo replays it");
    caret
}

// ---- Which blocks ----

#[test]
fn the_menus_block_alone_unless_it_is_part_of_the_selection() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "a child"), p(0, "b"), p(0, "c")]);
    editor.set_caret(at(ids[2], 0)).unwrap();
    assert_eq!(
        editor.action_targets(ids[0]).unwrap(),
        vec![ids[0]],
        "a caret selects nothing"
    );
    editor.select_between(at(ids[0], 0), at(ids[2], 1)).unwrap();
    assert_eq!(
        editor.action_targets(ids[1]).unwrap(),
        vec![ids[0], ids[1], ids[2]],
        "every block the text selection touches"
    );
    assert_eq!(editor.action_targets(ids[3]).unwrap(), vec![ids[3]], "outside it");
    editor.select_blocks(&[ids[0], ids[3]]).unwrap();
    assert_eq!(
        editor.action_targets(ids[1]).unwrap(),
        vec![ids[0], ids[3]],
        "nested inside a selected block counts as part of it"
    );
    assert_eq!(editor.action_targets(ids[2]).unwrap(), vec![ids[2]]);
}

// ---- Turn into ----

#[test]
fn turn_into_converts_one_block_or_the_whole_selection_in_one_step() {
    let (mut editor, ids) = page(&[p(0, "one"), p(0, "two"), p(0, "three")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    act_and_undo(&mut editor, ids[2], BlockAction::TurnInto(BlockKind::Quote));
    assert_eq!(outline(&editor)[2], row(0, BlockKind::Quote, "three"));

    editor.select_blocks(&[ids[0], ids[1]]).unwrap();
    act_and_undo(
        &mut editor,
        ids[1],
        BlockAction::TurnInto(BlockKind::Todo { checked: false }),
    );
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Todo { checked: false }, "one"),
            row(0, BlockKind::Todo { checked: false }, "two"),
            row(0, BlockKind::Quote, "three"),
        ]
    );
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Blocks(vec![ids[0], ids[1]])),
        "the selection stays"
    );
}

#[test]
fn turn_into_a_divider_skips_blocks_that_would_lose_their_text() {
    let (mut editor, ids) = page(&[p(0, "words"), p(0, "")]);
    editor.select_blocks(&ids).unwrap();
    editor
        .block_action(ids[0], BlockAction::TurnInto(BlockKind::Divider))
        .unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "words"),
            row(0, BlockKind::Divider, "")
        ]
    );
}

#[test]
fn turn_into_code_drops_formatting_but_keeps_the_text() {
    let (mut editor, ids) = page(&[p(0, "bold")]);
    editor
        .doc
        .apply(PageCommand::SetMark {
            id: ids[0],
            range: 0..4,
            mark: Mark::Bold,
        })
        .unwrap();
    editor
        .block_action(
            ids[0],
            BlockAction::TurnInto(BlockKind::Code {
                language: String::new(),
            }),
        )
        .unwrap();
    let spans = editor.doc().text(ids[0]).unwrap().unwrap();
    assert_eq!(
        (spans.len(), spans[0].text.as_str(), spans[0].marks.bold),
        (1, "bold", false)
    );
}

// ---- Duplicate ----

#[test]
fn duplicate_copies_a_block_with_its_children_and_selects_the_copy() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "parent"),
        (1, BlockKind::BulletedList, "child"),
        p(0, "after"),
    ]);
    editor.set_caret(at(ids[2], 2)).unwrap();
    act_and_undo(&mut editor, ids[0], BlockAction::Duplicate);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, "parent"),
            row(1, BlockKind::BulletedList, "child"),
            row(0, BlockKind::BulletedList, "parent"),
            row(1, BlockKind::BulletedList, "child"),
            row(0, BlockKind::Paragraph, "after"),
        ]
    );
    let copy = editor.rows().unwrap()[2].id;
    assert_ne!(copy, ids[0], "a new ID");
    // Undo and redo re-ran the action; run it once more to see the selection it leaves.
    editor.undo().unwrap();
    editor.set_caret(at(ids[2], 2)).unwrap();
    editor.block_action(ids[0], BlockAction::Duplicate).unwrap();
    let Some(Selection::Blocks(selected)) = editor.selection().unwrap() else {
        panic!("the copy should be selected");
    };
    assert_eq!(selected.len(), 1);
    assert_ne!(selected[0], ids[0]);
}

#[test]
fn duplicating_a_text_selection_copies_every_block_it_touches() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    editor.select_between(at(ids[0], 0), at(ids[1], 1)).unwrap();
    act_and_undo(&mut editor, ids[0], BlockAction::Duplicate);
    let texts: Vec<String> = outline(&editor).into_iter().map(|(_, _, t)| t).collect();
    assert_eq!(texts, vec!["a", "a", "b", "b", "c"]);
}

// ---- Delete ----

#[test]
fn delete_removes_the_block_or_the_selection_with_children_in_one_step() {
    let (mut editor, ids) = page(&[
        p(0, "keep"),
        p(0, "go"),
        p(1, "go child"),
        p(0, "also go"),
        p(0, "end"),
    ]);
    editor.set_caret(at(ids[4], 0)).unwrap();
    let caret = act_and_undo(&mut editor, ids[1], BlockAction::Delete);
    assert_eq!(caret, Some(at(ids[0], 4)), "the end of the block above");
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "keep"),
            row(0, BlockKind::Paragraph, "also go"),
            row(0, BlockKind::Paragraph, "end")
        ]
    );
    editor.undo().unwrap();
    editor.select_between(at(ids[1], 0), at(ids[3], 2)).unwrap();
    editor.block_action(ids[3], BlockAction::Delete).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "keep"),
            row(0, BlockKind::Paragraph, "end")
        ]
    );
}

#[test]
fn an_action_on_a_missing_block_does_nothing() {
    let (mut editor, ids) = page(&[p(0, "a")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    let gone = BlockId::new();
    assert_eq!(editor.action_targets(gone).unwrap(), Vec::<BlockId>::new());
    assert_eq!(
        editor.block_action(gone, BlockAction::Delete).unwrap(),
        Some(at(ids[0], 1))
    );
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "a")]);
}

// ---- Move up / Move down ----

fn texts(editor: &PageEditor) -> Vec<(usize, String)> {
    outline(editor).into_iter().map(|(d, _, t)| (d, t)).collect()
}

fn t(depth: usize, text: &str) -> (usize, String) {
    (depth, text.into())
}

#[test]
fn move_up_and_down_swap_with_a_sibling_taking_children_along() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(1, "b child"), p(0, "c")]);
    editor.set_caret(at(ids[1], 1)).unwrap();
    act_and_undo(&mut editor, ids[1], BlockAction::MoveUp);
    assert_eq!(
        texts(&editor),
        vec![t(0, "b"), t(1, "b child"), t(0, "a"), t(0, "c")]
    );
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[1], 1)),
        "the caret travels with the block"
    );
    act_and_undo(&mut editor, ids[1], BlockAction::MoveDown);
    act_and_undo(&mut editor, ids[1], BlockAction::MoveDown);
    assert_eq!(
        texts(&editor),
        vec![t(0, "a"), t(0, "c"), t(0, "b"), t(1, "b child")]
    );
}

#[test]
fn moves_stay_among_siblings_and_stop_at_the_edges() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "x"), p(1, "y"), p(0, "b")]);
    assert!(!editor.can_move(ids[0], true).unwrap(), "first on the page");
    assert!(editor.can_move(ids[0], false).unwrap());
    assert!(
        !editor.can_move(ids[1], true).unwrap(),
        "first among its siblings"
    );
    assert!(
        !editor.can_move(ids[2], false).unwrap(),
        "last among its siblings"
    );
    assert!(!editor.can_move(ids[3], false).unwrap(), "last on the page");
    let before = outline(&editor);
    editor.block_action(ids[3], BlockAction::MoveDown).unwrap();
    assert_eq!(outline(&editor), before, "nothing happens at the edge");
    assert!(!editor.undo().unwrap(), "and no undo step is left behind");

    editor.block_action(ids[2], BlockAction::MoveUp).unwrap();
    assert_eq!(
        texts(&editor),
        vec![t(0, "a"), t(1, "y"), t(1, "x"), t(0, "b")],
        "a nested block moves inside its parent"
    );
}

#[test]
fn a_selection_moves_together() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c"), p(0, "d")]);
    editor.select_blocks(&[ids[1], ids[2]]).unwrap();
    act_and_undo(&mut editor, ids[2], BlockAction::MoveDown);
    assert_eq!(texts(&editor), vec![t(0, "a"), t(0, "d"), t(0, "b"), t(0, "c")]);
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Blocks(vec![ids[1], ids[2]])),
        "the selection stays on the moved blocks"
    );
    act_and_undo(&mut editor, ids[1], BlockAction::MoveUp);
    act_and_undo(&mut editor, ids[1], BlockAction::MoveUp);
    assert_eq!(texts(&editor), vec![t(0, "b"), t(0, "c"), t(0, "a"), t(0, "d")]);
}

#[test]
fn a_selection_across_levels_does_not_move() {
    let (mut editor, ids) = page(&[p(0, "a"), p(1, "x"), p(0, "b"), p(0, "c")]);
    editor.select_between(at(ids[1], 0), at(ids[2], 1)).unwrap();
    assert!(!editor.can_move(ids[2], true).unwrap());
    assert!(!editor.can_move(ids[2], false).unwrap());
    // A parent with its child selected is just the parent.
    editor.select_between(at(ids[0], 0), at(ids[1], 1)).unwrap();
    assert!(editor.can_move(ids[0], false).unwrap());
    editor.block_action(ids[0], BlockAction::MoveDown).unwrap();
    assert_eq!(texts(&editor), vec![t(0, "b"), t(0, "a"), t(1, "x"), t(0, "c")]);
}
