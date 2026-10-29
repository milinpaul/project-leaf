//! Tests for `ReplaceText` (E04-S04): typing over a selection in one undoable step.

use leaf_core::BlockId;

use crate::{BlockKind, History, Mark, Marks, ModelError, PageCommand, PageDoc, PageOutcome, TextSpan};

fn page_with(kind: BlockKind, text: &str) -> (PageDoc, BlockId) {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let Ok(PageOutcome::BlockCreated(id)) = page.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind,
        text: text.into(),
    }) else {
        panic!("insert failed");
    };
    (page, id)
}

fn replace(page: &mut PageDoc, id: BlockId, range: std::ops::Range<usize>, text: &str) -> PageOutcome {
    page.apply(PageCommand::ReplaceText {
        id,
        range,
        text: text.into(),
    })
    .unwrap()
}

fn mark(page: &mut PageDoc, id: BlockId, range: std::ops::Range<usize>, mark: Mark) {
    page.apply(PageCommand::SetMark { id, range, mark }).unwrap();
}

fn spans(page: &PageDoc, id: BlockId) -> Vec<(String, Marks)> {
    page.text(id)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|TextSpan { text, marks }| (text, marks))
        .collect()
}

fn bold() -> Marks {
    Marks {
        bold: true,
        ..Marks::default()
    }
}

#[test]
fn replacing_uses_scalar_positions_and_undoes_in_one_step() {
    let (mut page, id) = page_with(BlockKind::Paragraph, "👋🏽 नमस्ते!");
    assert_eq!(replace(&mut page, id, 3..9, "hello"), PageOutcome::Changed);
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("👋🏽 hello!"));
    assert!(matches!(page.undo().unwrap(), History::Done { .. }));
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("👋🏽 नमस्ते!"));
    page.undo().unwrap();
    assert!(
        page.is_empty(),
        "the next undo step is the block's creation: one step, not two"
    );
    page.redo().unwrap();
    page.redo().unwrap();
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("👋🏽 hello!"));
}

#[test]
fn an_empty_range_inserts_and_empty_text_deletes() {
    let (mut page, id) = page_with(BlockKind::Paragraph, "abc");
    replace(&mut page, id, 1..1, "X");
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("aXbc"));
    replace(&mut page, id, 0..2, "");
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("bc"));
    assert_eq!(replace(&mut page, id, 1..1, ""), PageOutcome::Unchanged);
}

#[test]
fn the_new_text_takes_the_formatting_of_the_first_replaced_character() {
    // "plain BOLD tail": typing over the bold word stays bold, even though it starts right after
    // plain text, where typing would normally be plain.
    let (mut page, id) = page_with(BlockKind::Paragraph, "plain BOLD tail");
    mark(&mut page, id, 6..10, Mark::Bold);
    replace(&mut page, id, 6..10, "Big");
    assert_eq!(
        spans(&page, id),
        vec![
            ("plain ".into(), Marks::default()),
            ("Big".into(), bold()),
            (" tail".into(), Marks::default()),
        ]
    );
    // Replacing " t", right after the bold word: plain, though typing there would be bold.
    replace(&mut page, id, 9..11, "x");
    assert_eq!(
        spans(&page, id),
        vec![
            ("plain ".into(), Marks::default()),
            ("Big".into(), bold()),
            ("xail".into(), Marks::default()),
        ]
    );
}

#[test]
fn typing_over_link_text_keeps_the_link() {
    let (mut page, id) = page_with(BlockKind::Paragraph, "see docs here");
    mark(&mut page, id, 4..8, Mark::Link("https://leaf.app".into()));
    mark(&mut page, id, 4..6, Mark::Italic);
    replace(&mut page, id, 4..8, "the manual");
    let link = Marks {
        italic: true,
        link: Some("https://leaf.app".into()),
        ..Marks::default()
    };
    assert_eq!(
        spans(&page, id),
        vec![
            ("see ".into(), Marks::default()),
            ("the manual".into(), link),
            (" here".into(), Marks::default()),
        ]
    );
}

#[test]
fn code_blocks_take_replacements_without_marks() {
    let (mut page, id) = page_with(
        BlockKind::Code {
            language: String::new(),
        },
        "let a = 1;",
    );
    replace(&mut page, id, 4..5, "answer");
    assert_eq!(
        spans(&page, id),
        vec![("let answer = 1;".into(), Marks::default())]
    );
}

#[test]
fn bad_replacements_are_refused_and_write_nothing() {
    let (mut page, id) = page_with(BlockKind::Paragraph, "abc");
    let before = page.to_bytes().unwrap();
    let err = page.apply(PageCommand::ReplaceText {
        id,
        range: 2..9,
        text: "x".into(),
    });
    assert!(matches!(err, Err(ModelError::TextOutOfRange { end: 9, len: 3 })));
    let divider = {
        let Ok(PageOutcome::BlockCreated(d)) = page.apply(PageCommand::InsertBlock {
            parent: None,
            index: 1,
            kind: BlockKind::Divider,
            text: String::new(),
        }) else {
            panic!("insert failed");
        };
        d
    };
    assert!(matches!(
        page.apply(PageCommand::ReplaceText {
            id: divider,
            range: 0..0,
            text: "x".into(),
        }),
        Err(ModelError::TextNotAllowed { .. })
    ));
    assert_eq!(page.plain_text(id).unwrap().as_deref(), Some("abc"));
    assert_ne!(before.len(), 0);
}
