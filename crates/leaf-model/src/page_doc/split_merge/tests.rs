//! Tests for splitting and merging blocks (E02-S05).

use leaf_core::{AssetId, BlockId};

use crate::{BlockKind, Mark, Marks, ModelError, PageCommand, PageDoc, PageOutcome, TextSpan};

fn add(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    match page
        .apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn split(page: &mut PageDoc, id: BlockId, pos: usize) -> BlockId {
    match page.apply(PageCommand::SplitBlock { id, pos }).unwrap() {
        PageOutcome::BlockCreated(new) => new,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn merge(page: &mut PageDoc, id: BlockId) -> Result<PageOutcome, ModelError> {
    page.apply(PageCommand::MergeIntoPrevious { id })
}

fn text(page: &PageDoc, id: BlockId) -> String {
    page.plain_text(id).unwrap().unwrap()
}

fn spans(page: &PageDoc, id: BlockId) -> Vec<TextSpan> {
    page.text(id).unwrap().unwrap()
}

fn span(text: &str, marks: Marks) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks,
    }
}

fn bold() -> Marks {
    Marks {
        bold: true,
        ..Marks::default()
    }
}

fn link(href: &str) -> Marks {
    Marks {
        link: Some(href.into()),
        ..Marks::default()
    }
}

fn mark(page: &mut PageDoc, id: BlockId, range: std::ops::Range<usize>, mark: Mark) {
    page.apply(PageCommand::SetMark { id, range, mark }).unwrap();
}

#[test]
fn split_moves_the_text_after_the_cursor_into_a_new_block_below() {
    let mut page = PageDoc::new();
    let first = add(&mut page, None, BlockKind::Paragraph, "Hello world");
    let after = add(&mut page, None, BlockKind::Paragraph, "after");
    let new = split(&mut page, first, 5);
    assert_eq!(page.root_blocks(), vec![first, new, after]);
    assert_eq!(
        (text(&page, first), text(&page, new)),
        ("Hello".into(), " world".into())
    );

    let at_end = split(&mut page, first, 5);
    assert_eq!(
        text(&page, at_end),
        "",
        "splitting at the end gives an empty block"
    );
    let at_start = split(&mut page, new, 0);
    assert_eq!(
        (text(&page, new), text(&page, at_start)),
        (String::new(), " world".into())
    );
}

#[test]
fn split_keeps_formatting_on_both_halves_without_leaking_it() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "Hello world");
    mark(&mut page, id, 0..5, Mark::Bold);
    mark(&mut page, id, 6..11, Mark::Link("https://leaf.app".into()));

    let new = split(&mut page, id, 4); // inside the bold word
    assert_eq!(spans(&page, id), vec![span("Hell", bold())]);
    assert_eq!(
        spans(&page, new),
        vec![
            span("o", bold()),
            span(" ", Marks::default()),
            span("world", link("https://leaf.app"))
        ],
        "the plain space doesn't pick up bold from the text before it"
    );
}

#[test]
fn split_continues_lists_and_todos_and_turns_others_into_paragraphs() {
    let cases = [
        (BlockKind::BulletedList, BlockKind::BulletedList),
        (BlockKind::NumberedList, BlockKind::NumberedList),
        (
            BlockKind::Todo { checked: true },
            BlockKind::Todo { checked: false },
        ),
        (BlockKind::Heading { level: 1 }, BlockKind::Paragraph),
        (BlockKind::Quote, BlockKind::Paragraph),
        (
            BlockKind::Code {
                language: "rust".into(),
            },
            BlockKind::Paragraph,
        ),
        (BlockKind::Paragraph, BlockKind::Paragraph),
    ];
    for (kind, expected) in cases {
        let mut page = PageDoc::new();
        let id = add(&mut page, None, kind.clone(), "ab");
        let new = split(&mut page, id, 1);
        assert_eq!(page.kind(new).unwrap(), expected, "splitting a {kind:?}");
        assert_eq!(page.kind(id).unwrap(), kind, "the first half keeps its kind");
    }
}

#[test]
fn splitting_a_block_with_children_makes_the_new_block_its_first_child() {
    let mut page = PageDoc::new();
    let parent = add(&mut page, None, BlockKind::BulletedList, "parent tail");
    let child = add(&mut page, Some(parent), BlockKind::BulletedList, "child");
    let new = split(&mut page, parent, 6);
    assert_eq!(
        page.children(parent).unwrap(),
        vec![new, child],
        "reading order: parent, tail, child"
    );
    assert_eq!(text(&page, new), " tail");
}

#[test]
fn split_refuses_blocks_without_text_and_positions_past_the_end() {
    let mut page = PageDoc::new();
    let divider = add(&mut page, None, BlockKind::Divider, "");
    let para = add(&mut page, None, BlockKind::Paragraph, "abc");
    assert!(matches!(
        page.apply(PageCommand::SplitBlock { id: divider, pos: 0 }),
        Err(ModelError::TextNotAllowed { .. })
    ));
    assert!(matches!(
        page.apply(PageCommand::SplitBlock { id: para, pos: 4 }),
        Err(ModelError::TextOutOfRange { end: 4, len: 3 })
    ));
    assert_eq!(page.len(), 2);
}

#[test]
fn merge_appends_text_with_formatting_and_reports_the_join() {
    let mut page = PageDoc::new();
    let first = add(&mut page, None, BlockKind::Paragraph, "Hello");
    let second = add(&mut page, None, BlockKind::Quote, " world");
    mark(&mut page, first, 0..5, Mark::Bold);
    mark(&mut page, second, 1..6, Mark::Link("https://leaf.app".into()));

    assert_eq!(
        merge(&mut page, second).unwrap(),
        PageOutcome::BlockMerged { into: first, at: 5 }
    );
    assert_eq!(page.root_blocks(), vec![first]);
    assert!(!page.contains(second));
    assert_eq!(
        spans(&page, first),
        vec![
            span("Hello", bold()),
            span(" ", Marks::default()),
            span("world", link("https://leaf.app"))
        ],
        "bold doesn't spread onto the merged text"
    );
}

#[test]
fn merge_targets_the_block_above_in_reading_order_and_carries_children() {
    let mut page = PageDoc::new();
    let top = add(&mut page, None, BlockKind::BulletedList, "top");
    let nested = add(&mut page, Some(top), BlockKind::BulletedList, "nested");
    let next = add(&mut page, None, BlockKind::Paragraph, " next");
    let grandchild = add(&mut page, Some(next), BlockKind::Paragraph, "grandchild");

    // The block above `next` in reading order is `nested`, not its sibling `top`.
    assert_eq!(
        merge(&mut page, next).unwrap(),
        PageOutcome::BlockMerged { into: nested, at: 6 }
    );
    assert_eq!(text(&page, nested), "nested next");
    assert_eq!(
        page.children(nested).unwrap(),
        vec![grandchild],
        "children move to the block above"
    );
}

#[test]
fn merging_a_first_child_into_its_parent_keeps_its_children_in_place() {
    let mut page = PageDoc::new();
    let parent = add(&mut page, None, BlockKind::Paragraph, "parent");
    let child = add(&mut page, Some(parent), BlockKind::Paragraph, " child");
    let sibling = add(&mut page, Some(parent), BlockKind::Paragraph, "sibling");
    let inner = add(&mut page, Some(child), BlockKind::Paragraph, "inner");

    merge(&mut page, child).unwrap();
    assert_eq!(text(&page, parent), "parent child");
    assert_eq!(
        page.children(parent).unwrap(),
        vec![inner, sibling],
        "inner takes the child's place, before sibling"
    );
}

#[test]
fn merging_into_a_block_that_cannot_hold_children_lifts_them_into_place() {
    let mut page = PageDoc::new();
    let heading = add(&mut page, None, BlockKind::Heading { level: 2 }, "Title");
    let para = add(&mut page, None, BlockKind::Paragraph, "");
    let child = add(&mut page, Some(para), BlockKind::Paragraph, "child");
    merge(&mut page, para).unwrap();
    assert_eq!(page.root_blocks(), vec![heading, child]);
}

#[test]
fn merging_into_code_drops_formatting() {
    let mut page = PageDoc::new();
    let code = add(
        &mut page,
        None,
        BlockKind::Code {
            language: String::new(),
        },
        "let x = ",
    );
    let para = add(&mut page, None, BlockKind::Paragraph, "42");
    mark(&mut page, para, 0..2, Mark::Bold);
    merge(&mut page, para).unwrap();
    assert_eq!(spans(&page, code), vec![span("let x = 42", Marks::default())]);
}

#[test]
fn merging_onto_a_divider_or_image_is_refused_so_the_editor_can_select_it() {
    for kind in [
        BlockKind::Divider,
        BlockKind::Image {
            asset: AssetId::of(b"i"),
            width: None,
            alt: String::new(),
        },
    ] {
        let mut page = PageDoc::new();
        let target = add(&mut page, None, kind, "");
        let para = add(&mut page, None, BlockKind::Paragraph, "text");
        let err = merge(&mut page, para).unwrap_err();
        assert!(
            matches!(err, ModelError::MergeTargetNotText { target: t } if t == target),
            "{err}"
        );
        assert_eq!(
            err.to_string(),
            format!("block {target} has no text to merge into")
        );
        assert_eq!(text(&page, para), "text", "nothing changed");
    }
}

#[test]
fn merging_the_first_block_or_a_block_without_text_does_nothing_or_is_refused() {
    let mut page = PageDoc::new();
    let first = add(&mut page, None, BlockKind::Paragraph, "first");
    let divider = add(&mut page, None, BlockKind::Divider, "");
    assert_eq!(merge(&mut page, first).unwrap(), PageOutcome::Unchanged);
    assert!(matches!(
        merge(&mut page, divider),
        Err(ModelError::TextNotAllowed { .. })
    ));
    assert_eq!(page.len(), 2);
}

#[test]
fn split_then_merge_restores_the_text_and_formatting() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "👋🏽 नमस्ते world");
    mark(&mut page, id, 3..9, Mark::Italic);
    let before = spans(&page, id);
    let new = split(&mut page, id, 5);
    assert_eq!(
        merge(&mut page, new).unwrap(),
        PageOutcome::BlockMerged { into: id, at: 5 }
    );
    assert_eq!(spans(&page, id), before);
}
