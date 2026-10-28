//! Tests for the structural block commands: insert, delete, move (E02-S02).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::{AssetId, BlockId};

use crate::{BlockKind, ModelError, PageCommand, PageDoc, PageOutcome};

fn insert_at(
    page: &mut PageDoc,
    parent: Option<BlockId>,
    index: usize,
    kind: BlockKind,
    text: &str,
) -> BlockId {
    let command = PageCommand::InsertBlock {
        parent,
        index,
        kind,
        text: text.into(),
    };
    match page.apply(command).unwrap() {
        PageOutcome::BlockCreated(block) => block,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn para(page: &mut PageDoc, parent: Option<BlockId>, text: &str) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    insert_at(page, parent, index, BlockKind::Paragraph, text)
}

fn mv(ids: &[BlockId], new_parent: Option<BlockId>, index: usize) -> PageCommand {
    PageCommand::MoveBlocks {
        ids: ids.to_vec(),
        new_parent,
        index,
    }
}

fn commit_counter(page: &PageDoc) -> (Arc<AtomicUsize>, loro::Subscription) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let sub = page.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));
    (count, sub)
}

/// Three top-level paragraphs `a`, `b`, `c`.
fn abc() -> (PageDoc, BlockId, BlockId, BlockId) {
    let mut page = PageDoc::new();
    let [a, b, c] = ["a", "b", "c"].map(|t| para(&mut page, None, t));
    (page, a, b, c)
}

#[test]
fn insert_block_places_blocks_with_kind_and_text() {
    let (mut page, a, b, c) = abc();
    let first = insert_at(&mut page, None, 0, BlockKind::Heading { level: 2 }, "Title");
    assert_eq!(page.root_blocks(), vec![first, a, b, c]);
    let nested = insert_at(&mut page, Some(b), 0, BlockKind::Todo { checked: true }, "Task");
    assert_eq!(page.children(b).unwrap(), vec![nested]);
    assert_eq!(page.parent(nested).unwrap(), Some(b));
    assert_eq!(page.parent(b).unwrap(), None);
    assert_eq!(page.kind(nested).unwrap(), BlockKind::Todo { checked: true });
    assert_eq!(page.plain_text(nested).unwrap().as_deref(), Some("Task"));

    let empty = insert_at(&mut page, None, 0, BlockKind::Quote, "");
    assert_eq!(
        page.plain_text(empty).unwrap().as_deref(),
        Some(""),
        "text-bearing kinds always get a text container"
    );
    let divider = insert_at(&mut page, None, 0, BlockKind::Divider, "");
    assert_eq!(page.plain_text(divider).unwrap(), None);
    assert_eq!(page.len(), 7);
}

#[test]
fn insert_block_enforces_kinds_children_and_positions_writing_nothing() {
    let mut page = PageDoc::new();
    let heading = insert_at(&mut page, None, 0, BlockKind::Heading { level: 1 }, "H");
    let code = insert_at(
        &mut page,
        None,
        1,
        BlockKind::Code {
            language: String::new(),
        },
        "x",
    );
    let divider = insert_at(&mut page, None, 2, BlockKind::Divider, "");
    let image = insert_at(
        &mut page,
        None,
        3,
        BlockKind::Image {
            asset: AssetId::of(b"i"),
            width: None,
            alt: String::new(),
        },
        "",
    );
    let para = insert_at(&mut page, None, 4, BlockKind::Paragraph, "");
    let (commits, _sub) = commit_counter(&page);

    let attempt = |page: &mut PageDoc, parent, index, kind, text: &str| {
        page.apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: text.into(),
        })
    };
    for parent in [heading, code, divider, image] {
        let err = attempt(&mut page, Some(parent), 0, BlockKind::Paragraph, "").unwrap_err();
        assert!(
            matches!(err, ModelError::ChildrenNotAllowed { parent: p, .. } if p == parent),
            "{err}"
        );
    }
    assert!(matches!(
        attempt(&mut page, None, 0, BlockKind::Divider, "text"),
        Err(ModelError::TextNotAllowed { .. })
    ));
    for level in [0, 4] {
        assert!(matches!(
            attempt(&mut page, None, 0, BlockKind::Heading { level }, ""),
            Err(ModelError::InvalidKind(_))
        ));
    }
    let unknown = BlockKind::Unknown {
        type_name: "kanban".into(),
    };
    assert!(matches!(
        attempt(&mut page, None, 0, unknown, ""),
        Err(ModelError::InvalidKind(_))
    ));
    assert!(matches!(
        attempt(&mut page, None, 6, BlockKind::Paragraph, ""),
        Err(ModelError::IndexOutOfRange { index: 6, len: 5 })
    ));
    assert!(matches!(
        attempt(&mut page, Some(para), 1, BlockKind::Paragraph, ""),
        Err(ModelError::IndexOutOfRange { index: 1, len: 0 })
    ));
    let ghost = BlockId::new();
    assert!(
        matches!(attempt(&mut page, Some(ghost), 0, BlockKind::Paragraph, ""), Err(ModelError::BlockNotFound(b)) if b == ghost)
    );

    assert_eq!(
        commits.load(Ordering::SeqCst),
        0,
        "rejected inserts write nothing"
    );
    assert_eq!(page.len(), 5);
}

#[test]
fn delete_blocks_removes_subtrees_and_reports_them_in_document_order() {
    let (mut page, a, b, c) = abc();
    let b1 = para(&mut page, Some(b), "b1");
    let b1x = para(&mut page, Some(b1), "b1x");
    let a1 = para(&mut page, Some(a), "a1");

    // Listed out of order, and with b1 already inside b: reported once, in reading order.
    let outcome = page
        .apply(PageCommand::DeleteBlocks { ids: vec![b1, b, a1] })
        .unwrap();
    assert_eq!(outcome, PageOutcome::BlocksDeleted(vec![a1, b, b1, b1x]));
    assert_eq!(page.root_blocks(), vec![a, c]);
    assert_eq!(page.children(a).unwrap(), Vec::<BlockId>::new());
    for gone in [b, b1, b1x, a1] {
        assert!(!page.contains(gone));
    }
    assert_eq!(page.len(), 2);
}

#[test]
fn delete_blocks_with_nothing_or_unknown_ids_changes_nothing() {
    let (mut page, a, _, _) = abc();
    let (commits, _sub) = commit_counter(&page);
    assert_eq!(
        page.apply(PageCommand::DeleteBlocks { ids: vec![] }).unwrap(),
        PageOutcome::Unchanged
    );
    let ghost = BlockId::new();
    let err = page
        .apply(PageCommand::DeleteBlocks { ids: vec![a, ghost] })
        .unwrap_err();
    assert!(matches!(err, ModelError::BlockNotFound(b) if b == ghost));
    assert!(page.contains(a), "nothing is deleted when any id is unknown");
    assert_eq!(commits.load(Ordering::SeqCst), 0);
}

#[test]
fn move_blocks_keeps_document_order_and_counts_positions_without_the_moved_blocks() {
    let mut page = PageDoc::new();
    let [x, s1, y, s2, z] = ["x", "s1", "y", "s2", "z"].map(|t| para(&mut page, None, t));

    // Listed backwards, still moved in reading order; index 0 among the others (x, y, z).
    assert_eq!(page.apply(mv(&[s2, s1], None, 0)).unwrap(), PageOutcome::Changed);
    assert_eq!(page.root_blocks(), vec![s1, s2, x, y, z]);
    page.apply(mv(&[s1, s2], None, 3)).unwrap();
    assert_eq!(
        page.root_blocks(),
        vec![x, y, z, s1, s2],
        "the others' count puts them last"
    );
    page.apply(mv(&[s1, s2], None, 1)).unwrap();
    assert_eq!(page.root_blocks(), vec![x, s1, s2, y, z]);
}

#[test]
fn move_blocks_across_parents_takes_nested_blocks_along() {
    let mut page = PageDoc::new();
    let (a, b) = (para(&mut page, None, "a"), para(&mut page, None, "b"));
    let a1 = para(&mut page, Some(a), "a1");
    let a1x = para(&mut page, Some(a1), "a1x");
    let b1 = para(&mut page, Some(b), "b1");

    // a1x is inside a1, so it just travels with it.
    page.apply(mv(&[a1x, a1], Some(b), 0)).unwrap();
    assert_eq!(page.children(b).unwrap(), vec![a1, b1]);
    assert_eq!(page.children(a1).unwrap(), vec![a1x]);
    assert_eq!(page.children(a).unwrap(), Vec::<BlockId>::new());

    page.apply(mv(&[a1], None, 2)).unwrap();
    assert_eq!(page.root_blocks(), vec![a, b, a1], "out to the top level");

    page.apply(mv(&[a1], Some(a), 0)).unwrap();
    assert_eq!(page.children(a).unwrap(), vec![a1], "into an empty parent");
}

#[test]
fn move_blocks_refuses_bad_destinations_writing_nothing() {
    let mut page = PageDoc::new();
    let a = para(&mut page, None, "a");
    let a1 = para(&mut page, Some(a), "a1");
    let heading = insert_at(&mut page, None, 1, BlockKind::Heading { level: 1 }, "H");
    let (commits, _sub) = commit_counter(&page);

    for target in [a, a1] {
        let err = page.apply(mv(&[a], Some(target), 0)).unwrap_err();
        assert!(
            matches!(err, ModelError::BlockMoveIntoOwnSubtree { block, new_parent } if block == a && new_parent == target),
            "{err}"
        );
    }
    assert!(matches!(
        page.apply(mv(&[a1], Some(heading), 0)),
        Err(ModelError::ChildrenNotAllowed { .. })
    ));
    assert!(matches!(
        page.apply(mv(&[a1], None, 3)),
        Err(ModelError::IndexOutOfRange { index: 3, len: 2 })
    ));
    let ghost = BlockId::new();
    assert!(matches!(page.apply(mv(&[ghost], None, 0)), Err(ModelError::BlockNotFound(b)) if b == ghost));
    assert!(matches!(page.apply(mv(&[a1], Some(ghost), 0)), Err(ModelError::BlockNotFound(b)) if b == ghost));
    assert_eq!(page.apply(mv(&[], None, 0)).unwrap(), PageOutcome::Unchanged);

    assert_eq!(commits.load(Ordering::SeqCst), 0);
    assert_eq!(page.children(a).unwrap(), vec![a1]);
}

#[test]
fn each_page_command_is_exactly_one_commit_and_survives_reload() {
    let mut page = PageDoc::new();
    let (commits, _sub) = commit_counter(&page);
    let a = para(&mut page, None, "a");
    let b = para(&mut page, None, "b");
    let b1 = para(&mut page, Some(b), "b1");
    page.apply(mv(&[b1], Some(a), 0)).unwrap();
    page.apply(PageCommand::DeleteBlocks { ids: vec![b] }).unwrap();
    assert_eq!(commits.load(Ordering::SeqCst), 5);

    let loaded = PageDoc::from_bytes(&page.to_bytes().unwrap()).unwrap();
    assert_eq!(loaded.root_blocks(), vec![a]);
    assert_eq!(loaded.children(a).unwrap(), vec![b1]);
    assert_eq!(loaded.plain_text(b1).unwrap().as_deref(), Some("b1"));
}
