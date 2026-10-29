//! Tests for Markdown shortcuts (E05-S01): each shortcut, undo giving back the typed characters,
//! and firing only at the start of a block.

use leaf_core::{AssetId, BlockId};
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use crate::PageEditor;

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
    let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
    doc.set_undo_merge_interval(0);
    (PageEditor::new(doc), ids)
}

fn para(text: &str) -> (BlockKind, &str) {
    (BlockKind::Paragraph, text)
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

fn outline(editor: &PageEditor) -> Vec<(BlockKind, String)> {
    editor
        .rows()
        .unwrap()
        .into_iter()
        .map(|row| (editor.doc().kind(row.id).unwrap(), row.text.unwrap_or_default()))
        .collect()
}

fn code() -> BlockKind {
    BlockKind::Code {
        language: String::new(),
    }
}

/// Types `keys` one character at a time into an empty paragraph, like a keyboard does.
fn type_into_empty(keys: &str) -> (PageEditor, BlockId, Caret) {
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    let mut caret = at(ids[0], 0);
    for ch in keys.chars() {
        caret = editor.insert_text(&ch.to_string()).unwrap().unwrap();
    }
    (editor, ids[0], caret)
}

#[test]
fn each_shortcut_converts_the_block_and_removes_what_was_typed() {
    let cases = [
        ("# ", BlockKind::Heading { level: 1 }),
        ("## ", BlockKind::Heading { level: 2 }),
        ("### ", BlockKind::Heading { level: 3 }),
        ("- ", BlockKind::BulletedList),
        ("* ", BlockKind::BulletedList),
        ("1. ", BlockKind::NumberedList),
        ("12. ", BlockKind::NumberedList),
        ("[] ", BlockKind::Todo { checked: false }),
        ("> ", BlockKind::Quote),
        ("```", code()),
    ];
    for (keys, kind) in cases {
        let (editor, id, caret) = type_into_empty(keys);
        assert_eq!(outline(&editor), vec![(kind, String::new())], "{keys:?}");
        assert_eq!(caret, at(id, 0), "{keys:?}");
    }
}

#[test]
fn three_dashes_make_a_divider_and_a_paragraph_below_for_the_caret() {
    let (editor, id, caret) = type_into_empty("---");
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Divider, String::new()),
            (BlockKind::Paragraph, String::new())
        ]
    );
    assert_ne!(caret.block, id);
    assert_eq!(caret.at, 0);
}

#[test]
fn undo_right_after_a_conversion_gives_back_the_typed_characters() {
    for keys in ["# ", "- ", "1. ", "```", "---"] {
        let (mut editor, id, _) = type_into_empty(keys);
        assert!(editor.undo().unwrap());
        assert_eq!(
            outline(&editor),
            vec![(BlockKind::Paragraph, keys.to_owned())],
            "{keys:?}"
        );
        assert_eq!(
            editor.caret().unwrap(),
            Some(at(id, keys.chars().count())),
            "{keys:?}: the caret after them"
        );
        // Redo converts again.
        assert!(editor.redo().unwrap());
        assert_ne!(outline(&editor)[0].0, BlockKind::Paragraph, "{keys:?}");
    }
}

#[test]
fn the_conversion_is_its_own_step_even_when_typing_merges() {
    // With the real merge interval, quick typing merges into one step; the conversion still doesn't.
    let (mut editor, ids) = page(&[para("")]);
    editor.doc.set_undo_merge_interval(60_000);
    editor.set_caret(at(ids[0], 0)).unwrap();
    for ch in ["#", " ", "H", "i"] {
        editor.insert_text(ch).unwrap();
    }
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Heading { level: 1 }, "Hi".into())]
    );
    editor.undo().unwrap();
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Heading { level: 1 }, String::new())]
    );
    editor.undo().unwrap();
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, "# ".into())]);
    editor.undo().unwrap();
    assert_eq!(outline(&editor), vec![(BlockKind::Paragraph, String::new())]);
}

#[test]
fn text_after_the_caret_is_kept_and_becomes_the_blocks_text() {
    let (mut editor, ids) = page(&[para("Groceries")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("#").unwrap();
    assert_eq!(
        editor.insert_text(" ").unwrap(),
        Some(at(ids[0], 0)),
        "the caret stays at the start of the text"
    );
    assert_eq!(
        outline(&editor),
        vec![(BlockKind::Heading { level: 1 }, "Groceries".into())]
    );
    // ``` turns the rest into code; --- with text after it does nothing.
    let (mut editor, ids) = page(&[para("let a = 1;"), para("keep")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("```").unwrap();
    assert_eq!(outline(&editor)[0], (code(), "let a = 1;".into()));
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.insert_text("---").unwrap();
    assert_eq!(outline(&editor)[1], (BlockKind::Paragraph, "---keep".into()));
}

#[test]
fn shortcuts_fire_only_at_the_start_of_a_block() {
    let (mut editor, ids) = page(&[para("a"), para("x- y")]);
    editor.set_caret(at(ids[0], 1)).unwrap();
    editor.insert_text(" # ").unwrap();
    editor.set_caret(at(ids[1], 2)).unwrap();
    editor.insert_text(" ").unwrap();
    assert_eq!(
        outline(&editor),
        vec![
            (BlockKind::Paragraph, "a # ".into()),
            (BlockKind::Paragraph, "x-  y".into()),
        ]
    );
    // Not shortcuts: other text, too many hashes, a number with no dot, letters.
    for keys in ["#x ", "#### ", "1 ", "a. ", "1234567890. ", "-- ", "[x] "] {
        let (editor, _, _) = type_into_empty(keys);
        assert_eq!(
            outline(&editor),
            vec![(BlockKind::Paragraph, keys.to_owned())],
            "{keys:?}"
        );
    }
}

#[test]
fn shortcuts_switch_between_kinds_but_not_into_the_same_one() {
    let (mut editor, ids) = page(&[
        (BlockKind::BulletedList, ""),
        (BlockKind::Heading { level: 1 }, ""),
    ]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("[] ").unwrap();
    assert_eq!(
        outline(&editor)[0],
        (BlockKind::Todo { checked: false }, String::new())
    );
    editor.insert_text("[] ").unwrap();
    assert_eq!(
        outline(&editor)[0],
        (BlockKind::Todo { checked: false }, "[] ".into()),
        "already a to-do: typed as text"
    );
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.insert_text("## ").unwrap();
    assert_eq!(
        outline(&editor)[1],
        (BlockKind::Heading { level: 2 }, String::new())
    );
}

#[test]
fn code_blocks_and_image_captions_take_shortcuts_literally() {
    let image = BlockKind::Image {
        asset: AssetId::of(b"img"),
        width: None,
        alt: String::new(),
    };
    let (mut editor, ids) = page(&[(code(), ""), (image.clone(), "")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.insert_text("# ").unwrap();
    editor.set_caret(at(ids[1], 0)).unwrap();
    editor.insert_text("- ").unwrap();
    assert_eq!(
        outline(&editor),
        vec![(code(), "# ".into()), (image, "- ".into())]
    );
}

#[test]
fn a_shortcut_typed_over_a_selection_or_committed_by_an_input_method() {
    let (mut editor, ids) = page(&[para("old text")]);
    editor.select(ids[0], 0..4).unwrap();
    editor.insert_text("> ").unwrap();
    assert_eq!(outline(&editor), vec![(BlockKind::Quote, "text".into())]);
    let (mut editor, ids) = page(&[para("")]);
    editor.set_caret(at(ids[0], 0)).unwrap();
    editor.set_preedit("-", 1);
    editor.commit_preedit("- ").unwrap();
    assert_eq!(outline(&editor), vec![(BlockKind::BulletedList, String::new())]);
}
