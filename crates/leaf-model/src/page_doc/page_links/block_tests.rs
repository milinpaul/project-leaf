//! Tests for links to a block inside a page (E10-S08): a page link with a `block` mark.

use leaf_core::{BlockId, PageId};
use loro::{Container, ExpandType, LoroDoc, LoroText, StyleConfig, ValueOrContainer};

use super::super::PageDoc;
use super::super::tests::insert;
use crate::block_kind::keys;
use crate::text::marks;
use crate::{BlockKind, Mark, Marks, PAGE_LINK, PageCommand, PageOutcome, TextSpan};

fn link(page: &mut PageDoc, id: BlockId, pos: usize, target: PageId, block: BlockId) {
    let outcome = page
        .apply(PageCommand::InsertBlockLink {
            id,
            pos,
            page: target,
            block,
        })
        .unwrap();
    assert_eq!(outcome, PageOutcome::Changed);
}

fn spans(page: &PageDoc, id: BlockId) -> Vec<TextSpan> {
    page.text(id).unwrap().unwrap()
}

fn plain(text: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks::default(),
    }
}

fn linked(target: PageId, block: Option<BlockId>) -> TextSpan {
    TextSpan {
        text: PAGE_LINK.to_string(),
        marks: Marks {
            page: Some(target),
            block,
            ..Marks::default()
        },
    }
}

#[test]
fn a_block_link_is_a_page_link_that_also_names_the_block() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "See ");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 4, trip, day);
    assert_eq!(spans(&page, id), vec![plain("See "), linked(trip, Some(day))]);
    // Still a link to the page, so it counts as a backlink.
    assert_eq!(page.page_links(), vec![(id, trip)]);
    assert_eq!(page.block_links(), vec![(id, trip, day)]);
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn typing_beside_a_block_link_never_joins_it_and_undo_removes_it_whole() {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 1, trip, day);
    for pos in [2, 1] {
        page.apply(PageCommand::InsertText {
            id,
            pos,
            text: "x".into(),
        })
        .unwrap();
    }
    assert_eq!(
        spans(&page, id),
        vec![plain("ax"), linked(trip, Some(day)), plain("xb")]
    );
    page.undo().unwrap();
    page.undo().unwrap();
    page.undo().unwrap();
    assert_eq!(spans(&page, id), vec![plain("ab")]);
}

#[test]
fn neighbouring_block_links_stay_apart() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 0, trip, day);
    link(&mut page, id, 1, trip, day);
    page.apply(PageCommand::InsertText {
        id,
        pos: 1,
        text: "and".into(),
    })
    .unwrap();
    assert_eq!(
        spans(&page, id),
        vec![linked(trip, Some(day)), plain("and"), linked(trip, Some(day))]
    );
    assert_eq!(page.block_links().len(), 2);
}

#[test]
fn split_merge_and_duplicate_keep_the_block() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 1, trip, day);
    let PageOutcome::BlockCreated(tail) = page.apply(PageCommand::SplitBlock { id, pos: 1 }).unwrap() else {
        panic!("split makes a block");
    };
    assert_eq!(spans(&page, tail), vec![linked(trip, Some(day)), plain("b")]);
    page.apply(PageCommand::MergeIntoPrevious { id: tail }).unwrap();
    assert_eq!(
        spans(&page, id),
        vec![plain("a"), linked(trip, Some(day)), plain("b")]
    );
    page.apply(PageCommand::DuplicateBlocks { ids: vec![id] })
        .unwrap();
    assert_eq!(page.block_links().len(), 2);
    assert!(
        page.block_links()
            .iter()
            .all(|link| (link.1, link.2) == (trip, day))
    );
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn replacing_a_block_link_with_words_leaves_no_mark() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new(), BlockId::new());
    page.apply(PageCommand::ReplaceText {
        id,
        range: 1..2,
        text: "Trip".into(),
    })
    .unwrap();
    assert_eq!(spans(&page, id), vec![plain("aTripb")]);
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn pointing_a_block_link_at_another_page_drops_the_block() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "");
    link(&mut page, id, 0, PageId::new(), BlockId::new());
    let other = PageId::new();
    page.apply(PageCommand::SetMark {
        id,
        range: 0..1,
        mark: Mark::PageLink(other),
    })
    .unwrap();
    assert_eq!(spans(&page, id), vec![linked(other, None)]);
    assert_eq!(page.block_links(), Vec::new());
}

#[test]
fn turning_into_code_drops_block_links() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    link(&mut page, id, 1, PageId::new(), BlockId::new());
    page.apply(PageCommand::SetBlockKind {
        id,
        kind: BlockKind::Code {
            language: String::new(),
        },
    })
    .unwrap();
    assert_eq!(spans(&page, id), vec![plain("ab")]);
    assert_eq!(page.check_invariants(), Ok(()));
}

/// The block's text container in a raw Loro doc, as another version of Leaf would reach it.
fn raw_text(doc: &LoroDoc) -> LoroText {
    let tree = doc.get_tree("blocks");
    let node = tree.roots()[0];
    match tree.get_meta(node).unwrap().get(keys::TEXT) {
        Some(ValueOrContainer::Container(Container::Text(text))) => text,
        _ => panic!("a text block"),
    }
}

/// A copy of `page` opened by a version of Leaf that knows page links but not block links.
fn older_copy(page: &PageDoc) -> LoroDoc {
    let doc = LoroDoc::new();
    doc.set_peer_id(99).unwrap();
    doc.config_default_text_style(Some(StyleConfig {
        expand: ExpandType::None,
    }));
    doc.import(&page.to_bytes().unwrap()).unwrap();
    doc
}

#[test]
fn an_older_version_typing_beside_a_block_link_keeps_it() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 1, trip, day);

    let old = older_copy(&page);
    let text = raw_text(&old);
    text.insert(2, "!").unwrap();
    text.insert(1, "?").unwrap();
    text.mark(0..text.len_unicode(), "italic", true).unwrap();
    old.commit();

    page.import_changes(&old.export(loro::ExportMode::all_updates()).unwrap())
        .unwrap();
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.block_links(), vec![(id, trip, day)]);
}

#[test]
fn a_stray_block_mark_is_repaired_on_merge() {
    let mut page = PageDoc::new();
    let id = insert(&mut page, BlockKind::Paragraph, "ab");
    let (trip, day) = (PageId::new(), BlockId::new());
    link(&mut page, id, 1, trip, day);

    // Another version carries the block mark onto words, and strips the page mark but not the block.
    let old = older_copy(&page);
    let text = raw_text(&old);
    text.mark(0..1, marks::BLOCK, day.to_string()).unwrap();
    old.commit();

    page.import_changes(&old.export(loro::ExportMode::all_updates()).unwrap())
        .unwrap();
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(
        spans(&page, id),
        vec![plain("a"), linked(trip, Some(day)), plain("b")]
    );
}
