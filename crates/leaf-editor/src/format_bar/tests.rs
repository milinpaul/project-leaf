//! Tests for the floating formatting bar's model (E07-S13).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, MarkKind, PageCommand, PageDoc, PageOutcome};

use crate::{FormatBar, PageEditor};

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
    (PageEditor::new(doc), ids)
}

fn code() -> BlockKind {
    BlockKind::Code {
        language: String::new(),
    }
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

fn bar(editor: &mut PageEditor) -> Option<FormatBar> {
    editor.format_bar().unwrap()
}

/// The marks shown as on, by name.
fn active(bar: &FormatBar) -> Vec<&'static str> {
    bar.marks.iter().filter(|m| m.active).map(|m| m.label).collect()
}

#[test]
fn no_bar_without_selected_text() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "hello world")]);
    assert_eq!(bar(&mut editor), None, "nothing focused");
    editor.set_caret(at(ids[0], 3)).unwrap();
    assert_eq!(bar(&mut editor), None, "just a caret");
    editor.select_blocks(&[ids[0]]).unwrap();
    assert_eq!(bar(&mut editor), None, "whole blocks have the block menu instead");
}

#[test]
fn a_text_selection_offers_turn_into_and_every_mark() {
    let (mut editor, ids) = page(&[(BlockKind::Heading { level: 2 }, "hello world")]);
    editor.select(ids[0], 0..5).unwrap();
    let bar = bar(&mut editor).unwrap();
    assert_eq!((bar.start, bar.end), (at(ids[0], 0), at(ids[0], 5)));
    let marks: Vec<_> = bar.marks.iter().map(|m| (m.kind, m.label)).collect();
    assert_eq!(
        marks,
        [
            (MarkKind::Bold, "Bold"),
            (MarkKind::Italic, "Italic"),
            (MarkKind::Strike, "Strikethrough"),
            (MarkKind::Code, "Code"),
            (MarkKind::Link, "Link"),
        ]
    );
    assert_eq!(active(&bar), Vec::<&str>::new());
    assert_eq!(bar.kind_label, "Heading 2");
    let current: Vec<_> = bar.kinds.iter().filter(|k| k.current).map(|k| k.label).collect();
    assert_eq!(current, ["Heading 2"], "the selected block's kind is ticked");
    let all: Vec<_> = bar.kinds.iter().map(|k| k.kind.clone()).collect();
    assert_eq!(all, crate::turn_into_kinds(), "the block menu's list");
}

#[test]
fn a_mark_is_on_only_when_all_the_selection_has_it() {
    let (mut editor, ids) = page(&[(BlockKind::Paragraph, "hello world")]);
    editor.select(ids[0], 0..5).unwrap();
    editor.toggle_mark(MarkKind::Bold).unwrap();
    editor.toggle_mark(MarkKind::Italic).unwrap();
    assert_eq!(active(&bar(&mut editor).unwrap()), ["Bold", "Italic"]);
    editor.select(ids[0], 0..8).unwrap();
    assert_eq!(
        active(&bar(&mut editor).unwrap()),
        Vec::<&str>::new(),
        "half bold isn't bold"
    );
    editor.select(ids[0], 0..5).unwrap();
    editor.set_link("https://example.com").unwrap();
    assert_eq!(active(&bar(&mut editor).unwrap()), ["Bold", "Italic", "Link"]);
}

#[test]
fn across_blocks_the_bar_covers_them_all() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "first"),
        (BlockKind::Paragraph, "second"),
        (BlockKind::Quote, "third"),
    ]);
    editor.select_between(at(ids[0], 2), at(ids[1], 3)).unwrap();
    let bar = bar(&mut editor).unwrap();
    assert_eq!(bar.kind_label, "Text", "both are text");
    assert_eq!(bar.marks.len(), 5);

    editor.select_between(at(ids[0], 2), at(ids[2], 3)).unwrap();
    let bar = editor.format_bar().unwrap().unwrap();
    assert_eq!(bar.kind_label, "Turn into", "mixed kinds: no single name");
    assert!(bar.kinds.iter().all(|k| !k.current), "and nothing ticked");
    assert_eq!(bar.kinds.len(), crate::turn_into_kinds().len());
}

#[test]
fn code_blocks_take_no_marks_but_can_change_kind() {
    let (mut editor, ids) = page(&[(code(), "let x = 1;"), (BlockKind::Paragraph, "text")]);
    editor.select(ids[0], 0..3).unwrap();
    let only_code = bar(&mut editor).unwrap();
    assert!(only_code.marks.is_empty(), "nothing to mark in a code block");
    assert_eq!(only_code.kind_label, "Code");

    editor.select_between(at(ids[0], 0), at(ids[1], 2)).unwrap();
    assert_eq!(
        bar(&mut editor).unwrap().marks.len(),
        5,
        "with some text outside the code block, the marks are back"
    );
}

#[test]
fn turn_into_is_left_out_when_a_selected_block_cant_change_kind() {
    let (mut editor, ids) = page(&[
        (BlockKind::Paragraph, "before"),
        (BlockKind::Divider, ""),
        (BlockKind::Paragraph, "after"),
    ]);
    editor.select_between(at(ids[0], 1), at(ids[2], 2)).unwrap();
    let bar = bar(&mut editor).unwrap();
    assert!(bar.kinds.is_empty(), "a divider has no text to keep");
    assert_eq!(bar.kind_label, "Turn into");
    assert_eq!(bar.marks.len(), 5);
}
