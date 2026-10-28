//! Tests for the invariant checker: a clean page passes, and each rule broken by a raw write (the
//! way a bug would) is reported under the right number.

use leaf_core::{AssetId, BlockId};
use loro::{Container, ValueOrContainer};

use crate::block_kind::keys;
use crate::{BlockKind, InvariantViolation, PageCommand, PageDoc, PageOutcome};

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

/// The invariant numbers reported, sorted.
fn broken(page: &PageDoc) -> Vec<u8> {
    let mut numbers: Vec<u8> = page
        .check_invariants()
        .unwrap_err()
        .iter()
        .map(|v| v.invariant)
        .collect();
    numbers.sort_unstable();
    numbers
}

#[test]
fn a_page_built_with_commands_passes() {
    let mut page = PageDoc::new();
    assert_eq!(page.check_invariants(), Ok(()));
    let parent = add(&mut page, None, BlockKind::Paragraph, "parent");
    add(
        &mut page,
        Some(parent),
        BlockKind::Todo { checked: true },
        "child",
    );
    for kind in [
        BlockKind::Heading { level: 3 },
        BlockKind::Code {
            language: "rust".into(),
        },
        BlockKind::Divider,
        BlockKind::Image {
            asset: AssetId::of(b"i"),
            width: Some(10.0),
            alt: "a".into(),
        },
    ] {
        add(&mut page, None, kind, "");
    }
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn a_missing_type_breaks_invariant_1() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    meta(&page, id).delete(keys::TYPE).unwrap();
    assert_eq!(broken(&page), vec![1]);
}

#[test]
fn text_on_the_wrong_kind_breaks_invariant_2() {
    let mut page = PageDoc::new();
    let para = add(&mut page, None, BlockKind::Paragraph, "");
    let divider = add(&mut page, None, BlockKind::Divider, "");
    meta(&page, para).delete(keys::TEXT).unwrap();
    meta(&page, divider).ensure_mergeable_text(keys::TEXT).unwrap();
    assert_eq!(broken(&page), vec![2, 2]);
}

#[test]
fn children_under_a_heading_break_invariant_3() {
    let mut page = PageDoc::new();
    let heading = add(&mut page, None, BlockKind::Heading { level: 1 }, "H");
    let para = add(&mut page, None, BlockKind::Paragraph, "p");
    page.tree
        .mov_to(page.node(para).unwrap(), page.node(heading).unwrap(), 0)
        .unwrap();
    assert_eq!(broken(&page), vec![3]);
}

#[test]
fn a_bad_heading_level_breaks_invariant_4() {
    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Heading { level: 2 }, "H");
    let Some(ValueOrContainer::Container(Container::Map(props))) = meta(&page, id).get(keys::PROPS) else {
        panic!("heading has props");
    };
    props.insert(keys::LEVEL, 9_i64).unwrap();
    assert_eq!(broken(&page), vec![4]);
}

#[test]
fn marks_on_code_break_invariant_5() {
    let mut page = PageDoc::new();
    let id = add(
        &mut page,
        None,
        BlockKind::Code {
            language: String::new(),
        },
        "let x = 1;",
    );
    page.text_container(id)
        .unwrap()
        .unwrap()
        .mark(0..3, "bold", true)
        .unwrap();
    assert_eq!(broken(&page), vec![5]);
}

#[test]
fn bad_or_shared_ids_and_a_stale_index_break_invariant_8() {
    let mut page = PageDoc::new();
    let first = add(&mut page, None, BlockKind::Paragraph, "");
    let second = add(&mut page, None, BlockKind::Paragraph, "");
    meta(&page, second).insert(keys::ID, first.to_string()).unwrap();
    // `second` now claims `first`'s id: used twice, and not indexed to its node.
    assert_eq!(broken(&page), vec![8]);

    let mut page = PageDoc::new();
    let id = add(&mut page, None, BlockKind::Paragraph, "");
    meta(&page, id).insert(keys::ID, "nope").unwrap();
    assert_eq!(broken(&page), vec![8]);
    meta(&page, id).delete(keys::ID).unwrap();
    assert_eq!(broken(&page), vec![8]);

    let mut page = PageDoc::new();
    add(&mut page, None, BlockKind::Paragraph, "");
    let stray = page.tree.create(None::<loro::TreeID>).unwrap();
    let stray_meta = page.tree.get_meta(stray).unwrap();
    stray_meta.insert(keys::ID, BlockId::new().to_string()).unwrap();
    stray_meta.insert(keys::TYPE, "divider").unwrap();
    let numbers = broken(&page);
    assert!(
        numbers.iter().all(|&n| n == 8) && numbers.len() == 2,
        "not indexed, and the index count is off: {numbers:?}"
    );
}

#[test]
fn unknown_kinds_are_left_to_their_own_rules() {
    let mut page = PageDoc::new();
    let para = add(&mut page, None, BlockKind::Paragraph, "");
    let unknown = add(&mut page, None, BlockKind::Paragraph, "");
    meta(&page, unknown).insert(keys::TYPE, "kanban").unwrap();
    // A future kind with text and children breaks no rule this version knows.
    page.tree
        .mov_to(page.node(para).unwrap(), page.node(unknown).unwrap(), 0)
        .unwrap();
    assert_eq!(page.check_invariants(), Ok(()));
}

#[test]
fn violations_explain_themselves() {
    let violation = InvariantViolation {
        invariant: 5,
        node: "3@1".into(),
        detail: "code text has marks".into(),
    };
    assert_eq!(
        violation.to_string(),
        "invariant 5 broken at node 3@1: code text has marks"
    );
}
