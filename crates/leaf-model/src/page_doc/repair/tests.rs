//! Tests for repairing a page after a merge (E02-S09): each broken rule is put back without losing
//! what the user wrote, repairs are not undoable, and a merge that breaks a rule comes out repaired.

use leaf_core::BlockId;

use crate::block_kind::keys;
use crate::{BlockKind, History, PageCommand, PageDoc, PageOutcome};

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

fn meta(page: &PageDoc, id: BlockId) -> loro::LoroMap {
    page.tree.get_meta(page.node(id).unwrap()).unwrap()
}

fn code() -> BlockKind {
    BlockKind::Code {
        language: String::new(),
    }
}

#[test]
fn a_page_that_keeps_the_rules_is_left_alone() {
    let mut page = PageDoc::new();
    let list = add(&mut page, None, BlockKind::BulletedList, "a");
    add(&mut page, Some(list), BlockKind::Paragraph, "b");
    assert!(!page.repair().unwrap());
}

#[test]
fn children_step_out_of_blocks_that_cannot_hold_them_keeping_their_order() {
    let mut page = PageDoc::new();
    let heading = add(&mut page, None, BlockKind::Heading { level: 1 }, "H");
    let after = add(&mut page, None, BlockKind::Paragraph, "after");
    let x = add(&mut page, None, code(), "x");
    let y = add(&mut page, None, BlockKind::Paragraph, "y");
    let z = add(&mut page, None, BlockKind::Paragraph, "z");
    // A code block under the heading, with its own child: both levels need repairing.
    page.tree
        .mov_to(page.node(x).unwrap(), page.node(heading).unwrap(), 0)
        .unwrap();
    page.tree
        .mov_to(page.node(y).unwrap(), page.node(heading).unwrap(), 1)
        .unwrap();
    page.tree
        .mov_to(page.node(z).unwrap(), page.node(x).unwrap(), 0)
        .unwrap();
    page.index = crate::page_doc::BlockIndex::build(&page.tree).unwrap();
    assert!(page.check_invariants().is_err());

    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.root_blocks(), vec![heading, x, z, y, after]);
}

#[test]
fn marks_leave_code_and_text_blocks_get_their_text_back() {
    let mut page = PageDoc::new();
    let snippet = add(&mut page, None, code(), "let x = 1;");
    let para = add(&mut page, None, BlockKind::Paragraph, "");
    page.text_container(snippet)
        .unwrap()
        .unwrap()
        .mark(0..3, "bold", true)
        .unwrap();
    meta(&page, para).delete(keys::TEXT).unwrap();

    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    let spans = page.text(snippet).unwrap().unwrap();
    assert_eq!(
        (spans.len(), spans[0].text.as_str()),
        (1, "let x = 1;"),
        "the words stay"
    );
    assert_eq!(page.plain_text(para).unwrap(), Some(String::new()));
}

#[test]
fn a_divider_holding_words_becomes_a_paragraph_and_an_empty_one_loses_its_text() {
    let mut page = PageDoc::new();
    let worded = add(&mut page, None, BlockKind::Divider, "");
    let empty = add(&mut page, None, BlockKind::Divider, "");
    meta(&page, worded)
        .ensure_mergeable_text(keys::TEXT)
        .unwrap()
        .insert(0, "kept")
        .unwrap();
    meta(&page, empty).ensure_mergeable_text(keys::TEXT).unwrap();

    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.kind(worded).unwrap(), BlockKind::Paragraph);
    assert_eq!(page.plain_text(worded).unwrap(), Some("kept".into()));
    assert_eq!(page.kind(empty).unwrap(), BlockKind::Divider);
}

#[test]
fn a_merge_that_breaks_a_rule_is_repaired_on_both_copies_and_not_undoable() {
    let mut a = PageDoc::new();
    a.set_undo_merge_interval(0);
    let block = add(&mut a, None, BlockKind::Paragraph, "fn main() {}");
    let mut b = PageDoc::from_bytes(&a.to_bytes().unwrap()).unwrap();
    b.set_undo_merge_interval(0);
    // At the same time: A turns the block into code; B nests a block under it and bolds a word.
    a.apply(PageCommand::SetBlockKind {
        id: block,
        kind: code(),
    })
    .unwrap();
    let child = add(&mut b, Some(block), BlockKind::Paragraph, "child");
    b.apply(PageCommand::SetMark {
        id: block,
        range: 0..2,
        mark: crate::Mark::Bold,
    })
    .unwrap();

    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    // Each repaired on its own; swapping the repairs changes nothing more.
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    for page in [&a, &b] {
        assert_eq!(page.check_invariants(), Ok(()));
        assert_eq!(page.root_blocks(), vec![block, child]);
        assert_eq!(page.kind(block).unwrap(), code());
    }

    // Undo on A undoes A's own change, not the repair: the block goes back to a paragraph, and the
    // child stays where the repair put it.
    assert!(matches!(a.undo().unwrap(), History::Done { .. }));
    assert_eq!(a.kind(block).unwrap(), BlockKind::Paragraph);
    assert_eq!(a.root_blocks(), vec![block, child]);
    // Next is A's insert of the block; then nothing: the repair never was a step.
    assert!(matches!(a.undo().unwrap(), History::Done { .. }));
    assert_eq!(a.root_blocks(), vec![child]);
    assert_eq!(a.undo().unwrap(), History::Nothing);
}

#[test]
fn a_heading_without_its_level_and_an_image_without_its_file_are_repaired() {
    let mut page = PageDoc::new();
    let heading = add(&mut page, None, BlockKind::Heading { level: 3 }, "Title");
    let image = add(
        &mut page,
        None,
        BlockKind::Image {
            asset: leaf_core::AssetId::of(b"img"),
            width: None,
            alt: String::new(),
        },
        "caption",
    );
    meta(&page, heading).delete(keys::LEVEL).unwrap();
    meta(&page, image).delete(keys::ASSET).unwrap();
    assert!(page.check_invariants().is_err());

    assert!(page.repair().unwrap());
    assert_eq!(page.check_invariants(), Ok(()));
    assert_eq!(page.kind(heading).unwrap(), BlockKind::Heading { level: 1 });
    assert_eq!(page.kind(image).unwrap(), BlockKind::Paragraph);
    assert_eq!(
        page.plain_text(image).unwrap(),
        Some("caption".into()),
        "the caption stays"
    );
    assert!(
        meta(&page, image).get(keys::ALT).is_none(),
        "no image settings left on a paragraph"
    );
}
