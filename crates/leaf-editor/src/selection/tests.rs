//! Tests for selection (E04-S06): Shift+arrows across blocks, Cmd/Ctrl+A, block selection mode, and
//! typing or keys over a selection that spans blocks.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, History, PageCommand, PageDoc, PageOutcome};

use super::Selection;
use crate::{Key, KeyOutcome, Motion, PageEditor, Vertical};

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

fn p(depth: usize, text: &str) -> (usize, BlockKind, &str) {
    (depth, BlockKind::Paragraph, text)
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// `Some(text selection)`, wrapped to compare with `selection()` directly.
#[expect(
    clippy::unnecessary_wraps,
    reason = "wrapped to compare with `selection()` directly"
)]
fn text(start: Caret, end: Caret) -> Option<Selection> {
    Some(Selection::Text { start, end })
}

fn outline(editor: &PageEditor) -> Vec<(usize, BlockKind, String)> {
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

fn row(depth: usize, kind: BlockKind, text: &str) -> (usize, BlockKind, String) {
    (depth, kind, text.into())
}

fn para(depth: usize, text: &str) -> (usize, BlockKind, String) {
    row(depth, BlockKind::Paragraph, text)
}

/// Undoes once and checks the page is back to `before`.
fn undo_restores(editor: &mut PageEditor, before: &[(usize, BlockKind, String)]) {
    assert!(matches!(editor.doc.undo().unwrap(), History::Done { .. }));
    assert_eq!(outline(editor), before, "one undo step restores the page");
}

// ---- Shift+arrows ----

#[test]
fn shift_arrows_extend_across_blocks_and_back() {
    let (mut editor, ids) = page(&[p(0, "ab"), p(0, "cd")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    assert_eq!(
        editor.extend(Motion::Right).unwrap(),
        text(at(ids[0], 1), at(ids[0], 2))
    );
    assert_eq!(
        editor.extend(Motion::Right).unwrap(),
        text(at(ids[0], 1), at(ids[1], 0)),
        "over the block boundary"
    );
    editor.extend(Motion::LineEnd).unwrap();
    assert_eq!(editor.selection().unwrap(), text(at(ids[0], 1), at(ids[1], 2)));
    // Back past the start: the selection flips to before the tail.
    for _ in 0..5 {
        editor.extend(Motion::Left).unwrap();
    }
    assert_eq!(editor.selection().unwrap(), text(at(ids[0], 0), at(ids[0], 1)));
    assert_eq!(editor.caret().unwrap(), Some(at(ids[0], 0)));
}

#[test]
fn extending_back_to_the_tail_selects_nothing_and_needs_a_caret() {
    let (mut editor, ids) = page(&[p(0, "ab")]);
    assert_eq!(editor.extend(Motion::Right).unwrap(), None, "no caret yet");
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.extend(Motion::Right).unwrap();
    assert_eq!(editor.extend(Motion::Left).unwrap(), None);
}

#[test]
fn plain_left_and_right_collapse_to_the_selection_edges() {
    let (mut editor, ids) = page(&[p(0, "ab"), p(0, "cd")]);
    editor.select_between(at(ids[0], 1), at(ids[1], 1)).unwrap();
    assert_eq!(editor.move_caret(Motion::Left).unwrap(), Some(at(ids[0], 1)));
    assert_eq!(editor.selection().unwrap(), None);
    editor.select_between(at(ids[1], 1), at(ids[0], 1)).unwrap();
    assert_eq!(editor.move_caret(Motion::Right).unwrap(), Some(at(ids[1], 1)));
    editor.select_between(at(ids[0], 1), at(ids[1], 1)).unwrap();
    assert_eq!(
        editor.move_caret(Motion::LineStart).unwrap(),
        Some(at(ids[1], 0)),
        "other moves go from the caret"
    );
}

/// One line per block, one column per character: enough for Shift+Up/Down.
struct Lines;

impl crate::LineLayout for Lines {
    fn line_count(&self, _: BlockId) -> usize {
        1
    }
    fn line_of(&self, _: BlockId, _: usize) -> usize {
        0
    }
    fn offset_to_x(&self, _: BlockId, offset: usize) -> f32 {
        f32::from(u16::try_from(offset).unwrap())
    }
    fn x_to_offset(&self, _: BlockId, _: usize, x: f32) -> usize {
        // Columns are whole numbers in these tests; the texts are short.
        (0..20usize)
            .find(|&o| f32::from(u16::try_from(o).unwrap()) >= x)
            .unwrap_or(0)
    }
}

#[test]
fn shift_up_and_down_extend_by_lines() {
    let (mut editor, ids) = page(&[p(0, "alpha"), p(0, "beta"), p(0, "gamma")]);
    editor.set_caret(at(ids[1], 2)).unwrap();
    assert_eq!(
        editor.extend_vertically(Vertical::Down, &Lines).unwrap(),
        text(at(ids[1], 2), at(ids[2], 2))
    );
    editor.extend_vertically(Vertical::Up, &Lines).unwrap();
    assert_eq!(
        editor.extend_vertically(Vertical::Up, &Lines).unwrap(),
        text(at(ids[0], 2), at(ids[1], 2))
    );
    let empty = PageEditor::new(PageDoc::new()).extend_vertically(Vertical::Up, &Lines);
    assert_eq!(empty.unwrap(), None);
}

#[test]
fn a_selection_follows_edits_and_ends_when_its_tail_block_goes() {
    let (mut editor, ids) = page(&[p(0, "ab"), p(0, "cd"), p(0, "ef")]);
    editor.select_between(at(ids[0], 1), at(ids[2], 1)).unwrap();
    editor
        .doc
        .apply(PageCommand::InsertText {
            id: ids[0],
            pos: 0,
            text: ">".into(),
        })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), text(at(ids[0], 2), at(ids[2], 1)));
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[0]] })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), None);
}

// ---- Cmd/Ctrl+A ----

#[test]
fn select_all_first_selects_the_block_text_then_every_block() {
    let (mut editor, ids) = page(&[p(0, "one"), p(1, "nested"), p(0, "two")]);
    editor.set_caret(at(ids[1], 2)).unwrap();
    assert_eq!(editor.select_all().unwrap(), text(at(ids[1], 0), at(ids[1], 6)));
    assert_eq!(
        editor.select_all().unwrap(),
        Some(Selection::Blocks(vec![ids[0], ids[2]])),
        "every top-level block, with what is nested in them"
    );
    assert_eq!(
        editor.select_all().unwrap(),
        Some(Selection::Blocks(vec![ids[0], ids[2]])),
        "and stays there"
    );
    editor.move_caret(Motion::Right).unwrap();
    assert_eq!(
        editor.selection().unwrap(),
        None,
        "a caret move ends block selection"
    );
}

#[test]
fn select_all_in_an_empty_block_or_without_a_caret_selects_every_block() {
    let (mut editor, ids) = page(&[p(0, "text"), p(0, "")]);
    editor.set_caret(at(ids[1], 0)).unwrap();
    assert_eq!(editor.select_all().unwrap(), Some(Selection::Blocks(ids.clone())));
    let (mut fresh, ids) = page(&[p(0, "x")]);
    assert_eq!(fresh.select_all().unwrap(), Some(Selection::Blocks(ids)));
}

// ---- Block selection mode ----

#[test]
fn backspace_on_selected_blocks_deletes_them_in_one_step() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(1, "b child"), p(0, "c")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    let before = outline(&editor);
    editor.select_blocks(&[ids[3], ids[1]]).unwrap();
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Blocks(vec![ids[1], ids[3]]))
    );
    assert_eq!(
        editor.press(Key::Backspace).unwrap(),
        KeyOutcome::Caret(at(ids[0], 1))
    );
    assert_eq!(outline(&editor), vec![para(0, "a")]);
    undo_restores(&mut editor, &before);
}

#[test]
fn deleting_every_block_leaves_one_empty_paragraph() {
    let (mut editor, ids) = page(&[p(0, "a"), (0, BlockKind::Divider, "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.select_all().unwrap();
    editor.select_all().unwrap();
    let KeyOutcome::Caret(caret) = editor.press(Key::Delete).unwrap() else {
        panic!("no caret");
    };
    assert_eq!(outline(&editor), vec![para(0, "")]);
    assert_eq!(caret.at, 0);
}

#[test]
fn tab_nests_selected_blocks_and_keeps_them_selected() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.select_blocks(&[ids[1], ids[2]]).unwrap();
    assert_eq!(editor.press(Key::Tab).unwrap(), KeyOutcome::Nothing);
    assert_eq!(outline(&editor), vec![para(0, "a"), para(1, "b"), para(1, "c")]);
    assert_eq!(
        editor.selection().unwrap(),
        Some(Selection::Blocks(vec![ids[1], ids[2]]))
    );
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(outline(&editor)[1].0, 0);
    assert_eq!(editor.press(Key::Enter).unwrap(), KeyOutcome::Nothing);
    assert_eq!(
        editor.insert_text("x").unwrap(),
        Some(at(ids[0], 0)),
        "typing does nothing"
    );
    assert_eq!(outline(&editor)[0], para(0, "a"));
}

#[test]
fn backspace_after_a_divider_selects_it_and_backspace_again_deletes_it() {
    let (mut editor, ids) = page(&[p(0, "a"), (0, BlockKind::Divider, ""), p(0, "b")]);
    editor.set_caret(at(ids[2], 0)).unwrap();
    assert_eq!(
        editor.press(Key::Backspace).unwrap(),
        KeyOutcome::SelectBlock(ids[1])
    );
    assert_eq!(editor.selection().unwrap(), Some(Selection::Blocks(vec![ids[1]])));
    assert_eq!(
        editor.press(Key::Backspace).unwrap(),
        KeyOutcome::Caret(at(ids[0], 1))
    );
    assert_eq!(outline(&editor), vec![para(0, "a"), para(0, "b")]);
}

#[test]
fn selecting_no_blocks_or_missing_ones_ends_the_mode() {
    let (mut editor, ids) = page(&[p(0, "a")]);
    editor.select_blocks(&[ids[0]]).unwrap();
    editor.select_blocks(&[]).unwrap();
    assert_eq!(editor.selection().unwrap(), None);
    editor.select_blocks(&[ids[0]]).unwrap();
    editor
        .doc
        .apply(PageCommand::DeleteBlocks { ids: vec![ids[0]] })
        .unwrap();
    assert_eq!(editor.selection().unwrap(), None, "its blocks were deleted");
    assert_eq!(editor.delete_blocks(&[ids[0]]).unwrap(), None);
}

// ---- Typing and keys over a selection across blocks ----

#[test]
fn typing_over_a_selection_across_blocks_joins_them_in_one_undo_step() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::Heading { level: 2 }, "Hello there"),
        p(0, "middle"),
        (0, BlockKind::BulletedList, "gone"),
        p(0, "and the end"),
        p(1, "child of end"),
    ]);
    editor.select_between(at(ids[0], 5), at(ids[3], 3)).unwrap();
    let before = outline(&editor);
    assert_eq!(editor.insert_text(", world —").unwrap(), Some(at(ids[0], 14)));
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::Heading { level: 2 }, "Hello, world — the end"),
            para(0, "child of end"),
        ],
        "a heading can't hold children, so the last block's children take its place"
    );
    undo_restores(&mut editor, &before);
}

#[test]
fn a_selection_ending_inside_nested_blocks_keeps_what_follows() {
    // The selection ends inside "deep", nested two levels under "outer". Deleting "outer" outright
    // would delete "deep" too; instead its text goes and its other children move up.
    let (mut editor, ids) = page(&[
        p(0, "start here"),
        p(0, "outer"),
        p(1, "skipped"),
        p(1, "inner"),
        p(2, "deep text"),
        p(2, "after deep"),
        p(1, "after inner"),
    ]);
    editor.select_between(at(ids[0], 5), at(ids[4], 5)).unwrap();
    let before = outline(&editor);
    assert_eq!(editor.insert_text("!").unwrap(), Some(at(ids[0], 6)));
    assert_eq!(
        outline(&editor),
        vec![
            para(0, "start!text"),
            para(1, "after deep"),
            para(1, "after inner"),
        ]
    );
    undo_restores(&mut editor, &before);
}

#[test]
fn backspace_and_delete_remove_a_selection_across_blocks() {
    for key in [Key::Backspace, Key::Delete] {
        let (mut editor, ids) = page(&[p(0, "abc"), (0, BlockKind::Divider, ""), p(0, "def")]);
        editor.select_between(at(ids[2], 1), at(ids[0], 2)).unwrap();
        let before = outline(&editor);
        assert_eq!(editor.press(key).unwrap(), KeyOutcome::Caret(at(ids[0], 2)));
        assert_eq!(outline(&editor), vec![para(0, "abef")], "{key:?}");
        undo_restores(&mut editor, &before);
    }
}

#[test]
fn enter_over_a_selection_across_blocks_deletes_then_splits() {
    let (mut editor, ids) = page(&[(0, BlockKind::BulletedList, "one two"), p(0, "three four")]);
    editor.select_between(at(ids[0], 3), at(ids[1], 5)).unwrap();
    let before = outline(&editor);
    let KeyOutcome::Caret(caret) = editor.press(Key::Enter).unwrap() else {
        panic!("no caret");
    };
    assert_eq!(
        outline(&editor),
        vec![
            row(0, BlockKind::BulletedList, "one"),
            row(0, BlockKind::BulletedList, " four"),
        ]
    );
    assert_eq!(caret.at, 0);
    undo_restores(&mut editor, &before);
}

#[test]
fn enter_over_a_selection_from_code_or_a_caption() {
    let code = BlockKind::Code {
        language: String::new(),
    };
    let (mut editor, ids) = page(&[(0, code.clone(), "let a"), p(0, "b = 1")]);
    editor.select_between(at(ids[0], 5), at(ids[1], 1)).unwrap();
    assert_eq!(
        editor.press(Key::Enter).unwrap(),
        KeyOutcome::Caret(at(ids[0], 6))
    );
    assert_eq!(outline(&editor), vec![row(0, code, "let a\n = 1")]);

    let image = BlockKind::Image {
        asset: leaf_core::AssetId::of(b"img"),
        width: None,
        alt: String::new(),
    };
    let (mut editor, ids) = page(&[(0, image.clone(), "caption"), p(0, "text")]);
    editor.select_between(at(ids[0], 3), at(ids[1], 2)).unwrap();
    editor.press(Key::Enter).unwrap();
    assert_eq!(
        outline(&editor),
        vec![row(0, image, "cap"), para(0, ""), para(0, "xt")],
        "a caption takes no text in, and Enter adds a paragraph after the image"
    );
}

#[test]
fn tab_over_a_selection_nests_every_block_it_touches_and_keeps_it() {
    let (mut editor, ids) = page(&[p(0, "a"), p(0, "b"), p(0, "c")]);
    editor.select_between(at(ids[1], 0), at(ids[2], 1)).unwrap();
    assert_eq!(editor.press(Key::Tab).unwrap(), KeyOutcome::Caret(at(ids[2], 1)));
    assert_eq!(outline(&editor), vec![para(0, "a"), para(1, "b"), para(1, "c")]);
    assert_eq!(editor.selection().unwrap(), text(at(ids[1], 0), at(ids[2], 1)));
    editor.select_between(at(ids[2], 1), at(ids[1], 0)).unwrap();
    editor.press(Key::ShiftTab).unwrap();
    assert_eq!(
        editor.caret().unwrap(),
        Some(at(ids[1], 0)),
        "the caret stays the moving end"
    );
    assert_eq!(editor.selection().unwrap(), text(at(ids[1], 0), at(ids[2], 1)));
}

#[test]
fn a_composition_over_a_selection_across_blocks_shows_at_its_start() {
    let (mut editor, ids) = page(&[p(0, "ab"), p(0, "cd")]);
    editor.select_between(at(ids[0], 1), at(ids[1], 1)).unwrap();
    editor.set_preedit("か", 1);
    let preedit = editor.preedit().unwrap().unwrap();
    assert_eq!((preedit.at.block, preedit.at.range), (ids[0], 1..1));
    assert_eq!(
        outline(&editor),
        vec![para(0, "ab"), para(0, "cd")],
        "nothing written yet"
    );
    editor.commit_preedit("か").unwrap();
    assert_eq!(outline(&editor), vec![para(0, "aかd")]);
}

// ---- Property: deleting any selection keeps the text around it and the page valid ----

mod property {
    use proptest::prelude::*;

    use super::*;

    /// All text in reading order, blocks run together.
    fn joined(editor: &PageEditor) -> String {
        editor
            .rows()
            .unwrap()
            .into_iter()
            .filter_map(|row| row.text)
            .collect()
    }

    /// The caret `at` in `block` as an offset into [`joined`].
    fn global(editor: &PageEditor, caret: Caret) -> usize {
        let mut offset = 0;
        for row in editor.rows().unwrap() {
            if row.id == caret.block {
                return offset + caret.at;
            }
            offset += row.text.map_or(0, |t| t.chars().count());
        }
        offset
    }

    fn kind(choice: u8) -> BlockKind {
        match choice % 6 {
            0 | 1 => BlockKind::Paragraph,
            2 => BlockKind::BulletedList,
            3 => BlockKind::Heading { level: 1 },
            4 => BlockKind::Divider,
            _ => BlockKind::Code {
                language: String::new(),
            },
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn typing_over_any_selection_replaces_exactly_it(
            shape in prop::collection::vec((0usize..3, any::<u8>(), "[a-c]{0,4}"), 2..8),
            ends in (any::<prop::sample::Index>(), any::<prop::sample::Index>(), 0usize..5, 0usize..5),
        ) {
            // Depth may grow by at most one per block, and only under blocks that allow children.
            let mut rows: Vec<(usize, BlockKind, String)> = Vec::new();
            for (depth, choice, text) in shape {
                let kind = kind(choice);
                let max = rows.last().map_or(0, |(d, k, _)| if k.allows_children() { d + 1 } else { *d });
                let text = if kind.has_text() { text } else { String::new() };
                rows.push((depth.min(max), kind, text));
            }
            let spec: Vec<(usize, BlockKind, &str)> =
                rows.iter().map(|(d, k, t)| (*d, k.clone(), t.as_str())).collect();
            let (mut editor, ids) = page(&spec);
            let texts: Vec<(BlockId, usize)> = ids
                .iter()
                .zip(&rows)
                .filter(|(_, (_, k, _))| k.has_text())
                .map(|(&id, (_, _, t))| (id, t.chars().count()))
                .collect();
            prop_assume!(!texts.is_empty());
            let (a, b, at_a, at_b) = ends;
            let (block_a, len_a) = texts[a.index(texts.len())];
            let (block_b, len_b) = texts[b.index(texts.len())];
            let tail = at(block_a, at_a.min(len_a));
            let head = at(block_b, at_b.min(len_b));

            let before_outline = outline(&editor);
            let before = joined(&editor);
            let (from, to) = {
                let (x, y) = (global(&editor, tail), global(&editor, head));
                (x.min(y), x.max(y))
            };
            editor.select_between(tail, head).unwrap();
            editor.insert_text("X").unwrap();

            let chars: Vec<char> = before.chars().collect();
            let expected: String = chars[..from].iter().chain(['X'].iter()).chain(&chars[to..]).collect();
            prop_assert_eq!(joined(&editor), expected);
            prop_assert!(editor.doc().check_invariants().is_ok());
            editor.doc.undo().unwrap();
            prop_assert_eq!(outline(&editor), before_outline);
        }
    }
}

// ---- Double and triple click ----

#[test]
fn a_double_click_selects_the_word_under_it_whole() {
    let (mut editor, ids) = page(&[p(0, "Hello, नमस्ते world 👋🏽!")]);
    let b = ids[0];
    let word = |editor: &mut PageEditor, at: usize| editor.select_word_at(Caret { block: b, at }).unwrap();
    assert_eq!(
        word(&mut editor, 2),
        text(Caret { block: b, at: 0 }, Caret { block: b, at: 5 }),
        "Hello"
    );
    assert_eq!(
        word(&mut editor, 0),
        text(Caret { block: b, at: 0 }, Caret { block: b, at: 5 }),
        "its start"
    );
    assert_eq!(
        word(&mut editor, 8),
        text(Caret { block: b, at: 7 }, Caret { block: b, at: 13 }),
        "a Hindi word is one word"
    );
    assert_eq!(
        word(&mut editor, 13),
        text(Caret { block: b, at: 13 }, Caret { block: b, at: 14 }),
        "the space after it"
    );
    let emoji = word(&mut editor, 20);
    assert_eq!(
        emoji,
        text(Caret { block: b, at: 20 }, Caret { block: b, at: 22 }),
        "an emoji with its skin tone"
    );
    assert_eq!(
        word(&mut editor, 23),
        text(Caret { block: b, at: 22 }, Caret { block: b, at: 23 }),
        "past the end: the last segment"
    );
}

#[test]
fn a_double_click_in_an_empty_block_places_the_caret_and_a_triple_click_selects_the_block() {
    let (mut editor, ids) = page(&[p(0, ""), p(0, "two words")]);
    assert_eq!(
        editor.select_word_at(Caret { block: ids[0], at: 0 }).unwrap(),
        None
    );
    assert_eq!(editor.caret().unwrap(), Some(Caret { block: ids[0], at: 0 }));
    assert_eq!(
        editor.select_block_text(ids[1]).unwrap(),
        text(Caret { block: ids[1], at: 0 }, Caret { block: ids[1], at: 9 })
    );
    assert_eq!(
        editor.select_block_text(ids[0]).unwrap(),
        None,
        "nothing to select"
    );
}
