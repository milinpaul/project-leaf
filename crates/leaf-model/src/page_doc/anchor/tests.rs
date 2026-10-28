//! Tests for anchored carets (E04-S02): they follow local edits, other peers' edits and undo.

use leaf_core::BlockId;

use crate::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

fn page_with(text: &str) -> (PageDoc, BlockId) {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    let block = add(&mut page, BlockKind::Paragraph, text);
    (page, block)
}

fn add(page: &mut PageDoc, kind: BlockKind, text: &str) -> BlockId {
    let index = page.root_blocks().len();
    match page.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind,
        text: text.into(),
    }) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("insert failed: {other:?}"),
    }
}

fn insert(page: &mut PageDoc, id: BlockId, pos: usize, text: &str) {
    page.apply(PageCommand::InsertText {
        id,
        pos,
        text: text.into(),
    })
    .unwrap();
}

fn at(page: &PageDoc, anchor: &crate::Anchor) -> Option<usize> {
    page.resolve(anchor).unwrap().map(|caret| caret.at)
}

#[test]
fn an_anchor_moves_with_text_typed_before_it_and_stays_put_for_text_after_it() {
    let (mut page, block) = page_with("hello world");
    let anchor = page.anchor(Caret { block, at: 6 }).unwrap();
    assert_eq!(anchor.block(), block);
    insert(&mut page, block, 0, ">> ");
    assert_eq!(at(&page, &anchor), Some(9));
    insert(&mut page, block, 14, "!");
    assert_eq!(at(&page, &anchor), Some(9), "typing after the caret leaves it");
}

#[test]
fn deleting_the_text_around_an_anchor_puts_it_where_the_text_was() {
    let (mut page, block) = page_with("hello world");
    let anchor = page.anchor(Caret { block, at: 8 }).unwrap();
    page.apply(PageCommand::DeleteText {
        id: block,
        range: 5..10,
    })
    .unwrap();
    assert_eq!(page.plain_text(block).unwrap().as_deref(), Some("hellod"));
    assert_eq!(at(&page, &anchor), Some(5));
}

#[test]
fn positions_count_unicode_scalars_like_the_rest_of_the_model() {
    // Family emoji: man, ZWJ, woman, ZWJ, girl = five scalars, one grapheme.
    let (mut page, block) = page_with("👨\u{200d}👩\u{200d}👧 hi");
    let anchor = page.anchor(Caret { block, at: 6 }).unwrap();
    insert(&mut page, block, 0, "नमस्ते ");
    assert_eq!(at(&page, &anchor), Some(6 + "नमस्ते ".chars().count()));
}

#[test]
fn an_anchor_at_or_past_the_end_stays_at_the_end() {
    let (mut page, block) = page_with("abc");
    let anchor = page.anchor(Caret { block, at: 99 }).unwrap();
    assert_eq!(at(&page, &anchor), Some(3));
    insert(&mut page, block, 0, "x");
    assert_eq!(at(&page, &anchor), Some(4));
}

#[test]
fn blocks_without_text_anchor_at_zero_and_deleted_blocks_resolve_to_none() {
    let (mut page, text_block) = page_with("abc");
    let divider = add(&mut page, BlockKind::Divider, "");
    let anchor = page
        .anchor(Caret {
            block: divider,
            at: 4,
        })
        .unwrap();
    assert_eq!(at(&page, &anchor), Some(0));
    let gone = page
        .anchor(Caret {
            block: text_block,
            at: 1,
        })
        .unwrap();
    page.apply(PageCommand::DeleteBlocks {
        ids: vec![text_block],
    })
    .unwrap();
    assert_eq!(page.resolve(&gone).unwrap(), None);
    assert!(
        page.anchor(Caret {
            block: text_block,
            at: 0
        })
        .is_err()
    );
}

#[test]
fn undo_moves_the_anchor_back_and_a_re_created_block_keeps_the_old_position() {
    let (mut page, block) = page_with("hello world");
    let anchor = page.anchor(Caret { block, at: 6 }).unwrap();
    insert(&mut page, block, 0, "abc");
    assert_eq!(at(&page, &anchor), Some(9));
    page.undo().unwrap();
    assert_eq!(at(&page, &anchor), Some(6));

    let anchor = page.anchor(Caret { block, at: 4 }).unwrap();
    page.apply(PageCommand::DeleteBlocks { ids: vec![block] })
        .unwrap();
    assert_eq!(page.resolve(&anchor).unwrap(), None);
    page.undo().unwrap();
    assert_eq!(
        at(&page, &anchor),
        Some(4),
        "the block is back under a new container"
    );
}

#[test]
fn another_peers_edits_move_the_anchor_and_show_up_as_changes() {
    let (mut mine, block) = page_with("hello world");
    let mut theirs = PageDoc::from_bytes(&mine.to_bytes().unwrap()).unwrap();
    mine.take_changes();
    let anchor = mine.anchor(Caret { block, at: 6 }).unwrap();

    insert(&mut theirs, block, 0, "Oh, ");
    let added = add(&mut theirs, BlockKind::Paragraph, "from them");
    mine.import_changes(&theirs.unsaved_changes().unwrap().unwrap())
        .unwrap();

    assert_eq!(at(&mine, &anchor), Some(10));
    assert_eq!(mine.plain_text(added).unwrap().as_deref(), Some("from them"));
    let changes = mine.take_changes();
    assert!(changes.structure);
    assert!(changes.blocks.contains(&block) && changes.blocks.contains(&added));
}

#[test]
fn concurrent_typing_at_the_same_spot_keeps_the_caret_after_my_text() {
    let (mut mine, block) = page_with("ab");
    let mut theirs = PageDoc::from_bytes(&mine.to_bytes().unwrap()).unwrap();
    insert(&mut mine, block, 1, "X");
    let anchor = mine.anchor(Caret { block, at: 2 }).unwrap();
    insert(&mut theirs, block, 1, "Y");
    mine.import_changes(&theirs.unsaved_changes().unwrap().unwrap())
        .unwrap();
    let text = mine.plain_text(block).unwrap().unwrap();
    let caret = at(&mine, &anchor).unwrap();
    assert_eq!(
        text.chars().nth(caret),
        Some('b'),
        "the caret stays before the character it was before, in {text:?}"
    );
}

#[test]
fn importing_garbage_is_an_error() {
    let (mut page, _) = page_with("abc");
    assert!(page.import_changes(&[1, 2, 3]).is_err());
}
