//! Tests for Enter, Backspace, Delete and Tab (E04-S05): one per row of the "Editor behaviour" table
//! in `PRODUCT_SPEC.md` that these keys cover, then code blocks, dividers and images, and undo.
//! Arrow keys are tested in `caret/vertical`; paste comes with E05-S06.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, History, Mark, PageCommand, PageDoc, PageOutcome, SavedSelection};

use super::{Key, KeyOutcome};
use crate::PageEditor;

/// Builds a page from `(depth, kind, text)` rows, loaded from bytes so building it can't be undone.
pub(super) fn page(blocks: &[(usize, BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
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

fn para(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Paragraph, text)
}

fn code(text: &str) -> (usize, BlockKind, &str) {
    (
        0,
        BlockKind::Code {
            language: String::new(),
        },
        text,
    )
}

pub(super) fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// The page as (depth, kind, text) rows, to compare whole outcomes at once.
pub(super) fn outline(editor: &PageEditor) -> Vec<(usize, BlockKind, String)> {
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

pub(super) fn row(depth: usize, kind: BlockKind, text: &str) -> (usize, BlockKind, String) {
    (depth, kind, text.into())
}

pub(super) fn press(editor: &mut PageEditor, key: Key) -> KeyOutcome {
    editor.press(key).unwrap()
}

/// Presses `key` with the caret at `caret`, then checks undo restores the page and the caret.
pub(super) fn press_and_undo(editor: &mut PageEditor, caret: Caret, key: Key) -> KeyOutcome {
    editor.set_caret(caret).unwrap();
    let before = outline(editor);
    let outcome = press(editor, key);
    let after = outline(editor);
    if after != before {
        assert_eq!(
            editor.doc.undo().unwrap(),
            History::Done {
                selection: Some(SavedSelection::at(caret))
            },
            "{key:?}: one undo step, back to the caret before the key"
        );
        assert_eq!(outline(editor), before, "{key:?}: undo restores the page");
        editor.doc.redo().unwrap();
        assert_eq!(outline(editor), after, "{key:?}: redo replays it");
    }
    outcome
}

// ---- One test per table row ----

#[test]
fn row_enter_mid_text_splits_and_lists_continue_but_headings_do_not() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "milk eggs"),
        (0, BlockKind::Heading { level: 1 }, "Title here"),
        (0, BlockKind::Quote, "to be or"),
    ]);
    let outcome = press_and_undo(&mut editor, at(ids[0], 5), Key::Enter);
    let rows = editor.rows().unwrap();
    assert_eq!(outcome, KeyOutcome::Caret(at(rows[1].id, 0)));
    press_and_undo(&mut editor, at(ids[1], 5), Key::Enter);
    press_and_undo(&mut editor, at(ids[2], 5), Key::Enter);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, "milk "),
            row(0, BlockKind::BulletedList, "eggs"),
            row(0, BlockKind::Heading { level: 1 }, "Title"),
            row(0, BlockKind::Paragraph, " here"),
            row(0, BlockKind::Quote, "to be"),
            row(0, BlockKind::Paragraph, " or"),
        ]
    );
}

#[test]
fn row_enter_on_an_empty_list_item_outdents_then_becomes_a_paragraph() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::Todo { checked: false }, "parent"),
        (1, BlockKind::Todo { checked: false }, ""),
    ]);
    let empty = ids[1];
    assert_eq!(
        press_and_undo(&mut editor, at(empty, 0), Key::Enter),
        KeyOutcome::Caret(at(empty, 0))
    );
    assert_eq!(
        outline(&editor)[1],
        row(0, BlockKind::Todo { checked: false }, ""),
        "out one level"
    );
    press_and_undo(&mut editor, at(empty, 0), Key::Enter);
    assert_eq!(
        outline(&editor)[1],
        row(0, BlockKind::Paragraph, ""),
        "at the root: a paragraph"
    );
    assert_eq!(editor.doc().len(), 2, "no block added");
}

#[test]
fn row_enter_in_code_adds_a_newline_and_leaves_after_two_empty_lines() {
    let (mut editor, ids) = page(&[code("fn a() {}")]);
    let block = ids[0];
    assert_eq!(
        press_and_undo(&mut editor, at(block, 9), Key::Enter),
        KeyOutcome::Caret(at(block, 10))
    );
    press(&mut editor, Key::Enter);
    assert_eq!(outline(&editor), vec![row(0, code("").1, "fn a() {}\n\n")]);
    let KeyOutcome::Caret(out) = press_and_undo(&mut editor, at(block, 11), Key::Enter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![row(0, code("").1, "fn a() {}"), row(0, BlockKind::Paragraph, "")],
        "the two empty lines are removed, and a paragraph follows"
    );
    assert_ne!(out.block, block);
}

#[test]
fn row_backspace_at_the_start_of_a_non_paragraph_makes_it_a_paragraph() {
    let (mut editor, ids) = page(&[para("above"), (0, BlockKind::Heading { level: 2 }, "Heading")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 0), Key::Backspace),
        KeyOutcome::Caret(at(ids[1], 0))
    );
    assert_eq!(outline(&editor)[1], row(0, BlockKind::Paragraph, "Heading"));
}

#[test]
fn row_backspace_at_the_start_of_a_paragraph_merges_it_and_its_children_up() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "one"),
        para("two"),
        (1, BlockKind::Paragraph, "child"),
    ]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 0), Key::Backspace),
        KeyOutcome::Caret(at(ids[0], 3))
    );
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, "onetwo"),
            row(1, BlockKind::Paragraph, "child")
        ]
    );
}

#[test]
fn row_backspace_after_a_divider_or_image_selects_it() {
    let image = image();
    let (mut editor, ids) = page(&[(0, BlockKind::Divider, ""), para("a"), (0, image, ""), para("b")]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 0), Key::Backspace),
        KeyOutcome::SelectBlock(ids[0])
    );
    assert_eq!(
        press_and_undo(&mut editor, at(ids[3], 0), Key::Backspace),
        KeyOutcome::SelectBlock(ids[2])
    );
    assert_eq!(editor.doc().len(), 4, "nothing changed");
}

#[test]
fn row_delete_at_the_end_merges_the_next_block_in() {
    let (mut editor, ids) = page(&[
        para("one"),
        (0, BlockKind::Heading { level: 3 }, "two"),
        (0, BlockKind::BulletedList, "three"),
        (1, BlockKind::BulletedList, "nested"),
    ]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 3), Key::Delete),
        KeyOutcome::Caret(at(ids[0], 3))
    );
    press_and_undo(&mut editor, at(ids[0], 6), Key::Delete);
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "onetwothree"),
            row(1, BlockKind::BulletedList, "nested")
        ],
        "the merged block's children come along"
    );
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 11), Key::Delete),
        KeyOutcome::Caret(at(ids[0], 11)),
        "a first child merges into its parent"
    );
    assert_eq!(
        outline(&editor),
        vec![row(0, BlockKind::Paragraph, "onetwothreenested")]
    );
    assert_eq!(
        press(&mut editor, Key::Delete),
        KeyOutcome::Caret(at(ids[0], 17)),
        "at the end of the page: nothing"
    );
}

#[test]
fn row_tab_and_shift_tab_indent_and_outdent() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "a"),
        (0, BlockKind::BulletedList, "b"),
    ]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 1), Key::Tab),
        KeyOutcome::Caret(at(ids[1], 1))
    );
    assert_eq!(outline(&editor)[1].0, 1, "under the previous sibling");
    press_and_undo(&mut editor, at(ids[1], 1), Key::ShiftTab);
    assert_eq!(outline(&editor)[1].0, 0, "back to the parent's level");
    assert_eq!(
        press(&mut editor, Key::ShiftTab),
        KeyOutcome::Caret(at(ids[1], 1)),
        "at the root it stays"
    );
}

#[test]
fn row_deleting_a_block_deletes_its_children_in_one_undo_step() {
    let (mut editor, ids) = page(&[
        para("keep"),
        (0, BlockKind::BulletedList, "parent"),
        (1, BlockKind::BulletedList, "child"),
        (2, BlockKind::BulletedList, "grandchild"),
        para("after"),
    ]);
    editor.set_caret(at(ids[1], 2)).unwrap();
    let before = outline(&editor);
    assert_eq!(editor.delete_block(ids[1]).unwrap(), Some(at(ids[0], 4)));
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "keep"),
            row(0, BlockKind::Paragraph, "after")
        ]
    );
    assert_eq!(
        editor.doc.undo().unwrap(),
        History::Done {
            selection: Some(SavedSelection::at(at(ids[1], 2)))
        }
    );
    assert_eq!(outline(&editor), before);
}

// ---- Mid-text Backspace and Delete, selections ----

#[test]
fn backspace_and_delete_inside_text_remove_one_whole_character() {
    let family = "👨\u{200d}👩\u{200d}👧"; // 5 scalars, one character
    let (mut editor, ids) = page(&[para(&format!("a{family}b"))]);
    let b = ids[0];
    assert_eq!(
        press_and_undo(&mut editor, at(b, 6), Key::Backspace),
        KeyOutcome::Caret(at(b, 1))
    );
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "ab")]);
    editor.doc.undo().unwrap();
    assert_eq!(
        press_and_undo(&mut editor, at(b, 1), Key::Delete),
        KeyOutcome::Caret(at(b, 1))
    );
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "ab")]);
}

#[test]
fn backspace_and_delete_remove_the_selection() {
    let (mut editor, ids) = page(&[para("hello world")]);
    for key in [Key::Backspace, Key::Delete] {
        editor.select(ids[0], 5..11).unwrap();
        assert_eq!(press(&mut editor, key), KeyOutcome::Caret(at(ids[0], 5)));
        assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "hello")]);
        editor.doc.undo().unwrap();
    }
}

#[test]
fn enter_over_a_selection_replaces_it_in_one_undo_step() {
    let (mut editor, ids) = page(&[(0, BlockKind::NumberedList, "first XX second")]);
    editor.select(ids[0], 5..9).unwrap();
    let before = outline(&editor);
    let KeyOutcome::Caret(caret) = press(&mut editor, Key::Enter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::NumberedList, "first"),
            row(0, BlockKind::NumberedList, "second")
        ]
    );
    assert_eq!(caret.at, 0);
    editor.doc.undo().unwrap();
    assert_eq!(outline(&editor), before, "delete and split undo together");
}

#[test]
fn no_caret_no_change() {
    let (mut editor, _) = page(&[para("a")]);
    for key in [
        Key::Enter,
        Key::ModEnter,
        Key::Backspace,
        Key::Delete,
        Key::Tab,
        Key::ShiftTab,
    ] {
        assert_eq!(press(&mut editor, key), KeyOutcome::Nothing);
    }
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "a")]);
}

// ---- Code blocks ----

#[test]
fn cmd_enter_leaves_a_code_block_from_anywhere_and_does_nothing_elsewhere() {
    let (mut editor, ids) = page(&[code("let a;\nlet b;"), para("after")]);
    let KeyOutcome::Caret(out) = press_and_undo(&mut editor, at(ids[0], 3), Key::ModEnter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, code("").1, "let a;\nlet b;"),
            row(0, BlockKind::Paragraph, ""),
            row(0, BlockKind::Paragraph, "after"),
        ],
        "the code is untouched; a paragraph comes right after it"
    );
    assert_eq!(out.at, 0);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[1], 2), Key::ModEnter),
        KeyOutcome::Caret(at(ids[1], 2))
    );
}

#[test]
fn enter_in_code_keeps_going_in_the_middle_and_after_one_empty_line() {
    let (mut editor, ids) = page(&[code("a\n\nb"), code("x\n")]);
    // Two empty lines in the middle don't exit; only at the end.
    press_and_undo(&mut editor, at(ids[0], 2), Key::Enter);
    assert_eq!(outline(&editor)[0].2, "a\n\n\nb");
    // One trailing empty line: Enter just adds another.
    press_and_undo(&mut editor, at(ids[1], 2), Key::Enter);
    assert_eq!(outline(&editor)[1].2, "x\n\n");
    assert_eq!(editor.doc().len(), 2);
}

#[test]
fn enter_over_a_selection_in_code_replaces_it_with_a_newline() {
    let (mut editor, ids) = page(&[code("a;b;")]);
    editor.select(ids[0], 2..3).unwrap();
    assert_eq!(press(&mut editor, Key::Enter), KeyOutcome::Caret(at(ids[0], 3)));
    assert_eq!(outline(&editor)[0].2, "a;\n;");
}

#[test]
fn delete_at_the_end_of_code_pulls_the_next_text_in_without_marks() {
    let (mut editor, ids) = page(&[code("x = "), para("bold")]);
    editor
        .doc
        .apply(PageCommand::SetMark {
            id: ids[1],
            range: 0..4,
            mark: Mark::Bold,
        })
        .unwrap();
    press_and_undo(&mut editor, at(ids[0], 4), Key::Delete);
    let spans = editor.doc().text(ids[0]).unwrap().unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].text, "x = bold");
    assert!(!spans[0].marks.bold);
}

// ---- Dividers and images ----

fn image() -> BlockKind {
    BlockKind::Image {
        asset: AssetId::of(b"photo"),
        width: None,
        alt: String::new(),
    }
}

#[test]
fn delete_before_a_divider_or_image_selects_it() {
    let (mut editor, ids) = page(&[
        para("a"),
        (0, BlockKind::Divider, ""),
        para("b"),
        (0, image(), ""),
    ]);
    assert_eq!(
        press_and_undo(&mut editor, at(ids[0], 1), Key::Delete),
        KeyOutcome::SelectBlock(ids[1])
    );
    assert_eq!(
        press_and_undo(&mut editor, at(ids[2], 1), Key::Delete),
        KeyOutcome::SelectBlock(ids[3])
    );
    assert_eq!(editor.doc().len(), 4);
}

#[test]
fn a_selected_divider_is_deleted_and_the_caret_goes_above_or_below() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::Divider, ""),
        para("text"),
        (0, BlockKind::Divider, ""),
    ]);
    assert_eq!(
        editor.delete_block(ids[2]).unwrap(),
        Some(at(ids[1], 4)),
        "above: its end"
    );
    assert_eq!(
        editor.delete_block(ids[0]).unwrap(),
        Some(at(ids[1], 0)),
        "nothing above: below"
    );
    assert_eq!(editor.delete_block(ids[0]).unwrap(), None, "already gone");
}

#[test]
fn deleting_the_last_block_leaves_an_empty_paragraph_in_one_undo_step() {
    let (mut editor, ids) = page(&[(0, BlockKind::Divider, "")]);
    let caret = editor.delete_block(ids[0]).unwrap().unwrap();
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Paragraph, "")]);
    assert_eq!(caret.at, 0);
    editor.doc.undo().unwrap();
    assert_eq!(outline(&editor), vec![row(0, BlockKind::Divider, "")]);
}

#[test]
fn in_an_image_caption_enter_adds_a_paragraph_and_the_edges_stay_put() {
    let (mut editor, ids) = page(&[para("a"), (0, image(), "caption")]);
    let caption = ids[1];
    let KeyOutcome::Caret(below) = press_and_undo(&mut editor, at(caption, 3), Key::Enter) else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Paragraph, "a"),
            row(0, image(), "caption"),
            row(0, BlockKind::Paragraph, "")
        ],
        "the caption isn't split"
    );
    assert_eq!(below.at, 0);
    assert_eq!(
        press_and_undo(&mut editor, at(caption, 0), Key::Backspace),
        KeyOutcome::SelectBlock(caption),
        "Backspace at the caption's start selects the image"
    );
    assert_eq!(
        press_and_undo(&mut editor, at(caption, 7), Key::Delete),
        KeyOutcome::Caret(at(caption, 7)),
        "a caption takes no text in"
    );
    assert_eq!(editor.doc().len(), 3);
}

#[test]
fn enter_on_a_divider_adds_a_paragraph_below() {
    let (mut editor, ids) = page(&[(0, BlockKind::Divider, ""), para("b")]);
    let below = editor.enter_at(ids[0], 0).unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Divider, ""),
            row(0, BlockKind::Paragraph, ""),
            row(0, BlockKind::Paragraph, "b")
        ]
    );
    assert_eq!(below.at, 0);
}

mod tasks;
