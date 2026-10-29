//! Tests for `DuplicateBlocks` (E05-S03): deep copies with new IDs, nothing lost, one undo step.

use leaf_core::BlockId;
use loro::{LoroList, LoroMap, LoroValue};

use crate::block_kind::keys;
use crate::{BlockKind, Mark, ModelError, PageCommand, PageDoc, PageOutcome, TextSpan};

fn fresh() -> PageDoc {
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);
    page
}

fn add(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    let Ok(PageOutcome::BlockCreated(id)) = page.apply(PageCommand::InsertBlock {
        parent,
        index,
        kind,
        text: text.into(),
    }) else {
        panic!("insert failed");
    };
    id
}

fn duplicate(page: &mut PageDoc, ids: Vec<BlockId>) -> Vec<BlockId> {
    match page.apply(PageCommand::DuplicateBlocks { ids }).unwrap() {
        PageOutcome::BlocksCreated(created) => created,
        other => panic!("unexpected {other:?}"),
    }
}

/// The page as (depth, kind, formatted text) in reading order, IDs left out.
fn shape(page: &PageDoc) -> Vec<(usize, BlockKind, Option<Vec<TextSpan>>)> {
    fn walk(
        page: &PageDoc,
        blocks: Vec<BlockId>,
        depth: usize,
        out: &mut Vec<(usize, BlockKind, Option<Vec<TextSpan>>)>,
    ) {
        for block in blocks {
            out.push((depth, page.kind(block).unwrap(), page.text(block).unwrap()));
            walk(page, page.children(block).unwrap(), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(page, page.root_blocks(), 0, &mut out);
    out
}

fn meta(page: &PageDoc, id: BlockId) -> LoroMap {
    page.tree.get_meta(page.node(id).unwrap()).unwrap()
}

#[test]
fn a_copy_lands_right_after_its_original_with_children_formatting_and_new_ids() {
    let mut page = fresh();
    let first = add(&mut page, None, BlockKind::Todo { checked: true }, "buy milk");
    let child = add(&mut page, Some(first), BlockKind::Quote, "nested");
    let grandchild = add(&mut page, Some(child), BlockKind::Heading { level: 2 }, "deep");
    let after = add(&mut page, None, BlockKind::Paragraph, "after");
    page.apply(PageCommand::SetMark {
        id: first,
        range: 0..3,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id: grandchild,
        range: 0..4,
        mark: Mark::Link("https://leaf.app".into()),
    })
    .unwrap();
    let original = shape(&page);

    let created = duplicate(&mut page, vec![first]);
    assert_eq!(created.len(), 1);
    let copy = created[0];
    assert_eq!(
        page.root_blocks(),
        vec![first, copy, after],
        "right after the original"
    );
    let mut expected = original[..3].to_vec();
    expected.extend(original[..3].iter().cloned());
    expected.push(original[3].clone());
    assert_eq!(
        shape(&page),
        expected,
        "kind, props, marks and nesting all copied"
    );

    let copied_child = page.children(copy).unwrap()[0];
    let copied_grandchild = page.children(copied_child).unwrap()[0];
    let ids = [first, child, grandchild, copy, copied_child, copied_grandchild];
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 6, "every copy has a new ID");
    assert!(page.check_invariants().is_ok());

    // The copy is independent of the original.
    page.apply(PageCommand::InsertText {
        id: copied_grandchild,
        pos: 0,
        text: "x".into(),
    })
    .unwrap();
    assert_eq!(page.plain_text(grandchild).unwrap().as_deref(), Some("deep"));
}

#[test]
fn several_blocks_are_each_copied_once_and_undo_is_one_step() {
    let mut page = fresh();
    let a = add(&mut page, None, BlockKind::Paragraph, "a");
    let a1 = add(&mut page, Some(a), BlockKind::BulletedList, "a1");
    let b = add(&mut page, None, BlockKind::Divider, "");
    let c = add(&mut page, None, BlockKind::Quote, "c");
    let before = shape(&page);

    // a1 is inside a: copied with it, not again on its own. Listing order doesn't matter.
    let created = duplicate(&mut page, vec![c, a1, a, b]);
    assert_eq!(created.len(), 3);
    let roots = page.root_blocks();
    assert_eq!(roots, vec![a, created[0], b, created[1], c, created[2]]);
    assert_eq!(page.children(created[0]).unwrap().len(), 1);

    page.undo().unwrap();
    assert_eq!(shape(&page), before, "one undo step");
    assert_eq!(page.root_blocks(), vec![a, b, c]);
    page.redo().unwrap();
    assert_eq!(page.root_blocks().len(), 6);
}

#[test]
fn data_from_newer_versions_is_copied_too() {
    let mut page = fresh();
    let id = add(&mut page, None, BlockKind::Paragraph, "hello");
    // A newer Leaf stored a scalar, a nested map, a list and a second text on the block.
    let record = meta(&page, id);
    record.insert("future_flag", true).unwrap();
    let nested = record.insert_container("future_map", LoroMap::new()).unwrap();
    nested.insert("colour", "teal").unwrap();
    let list = record.insert_container("future_list", LoroList::new()).unwrap();
    list.insert(0, 7).unwrap();
    let inner = list.insert_container(1, LoroMap::new()).unwrap();
    inner.insert("k", "v").unwrap();
    let inner_list = list.insert_container(2, LoroList::new()).unwrap();
    inner_list.insert(0, "deep").unwrap();
    let note = list.insert_container(3, loro::LoroText::new()).unwrap();
    note.insert(0, "a note").unwrap();
    let caption = record
        .insert_container("future_text", loro::LoroText::new())
        .unwrap();
    caption.insert(0, "second text").unwrap();
    page.doc.commit();

    let copy = duplicate(&mut page, vec![id])[0];
    let original = meta(&page, id).get_deep_value();
    let copied = meta(&page, copy).get_deep_value();
    let without_id = |value: LoroValue| {
        let LoroValue::Map(map) = value else {
            panic!("not a map")
        };
        let mut map = (*map).clone();
        map.remove(keys::ID);
        map
    };
    assert_eq!(without_id(copied), without_id(original), "everything but the ID");
}

#[test]
fn nothing_listed_changes_nothing_and_unknown_blocks_are_refused() {
    let mut page = fresh();
    assert_eq!(
        page.apply(PageCommand::DuplicateBlocks { ids: Vec::new() })
            .unwrap(),
        PageOutcome::Unchanged
    );
    let missing = BlockId::new();
    assert!(matches!(
        page.apply(PageCommand::DuplicateBlocks { ids: vec![missing] }),
        Err(ModelError::BlockNotFound(id)) if id == missing
    ));
}
