//! Tests for indent and outdent (E02-S06).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::BlockId;

use crate::{BlockKind, PageCommand, PageDoc, PageOutcome};

fn add(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind) -> BlockId {
    let index = match parent {
        Some(p) => page.children(p).unwrap().len(),
        None => page.root_blocks().len(),
    };
    match page
        .apply(PageCommand::InsertBlock {
            parent,
            index,
            kind,
            text: String::new(),
        })
        .unwrap()
    {
        PageOutcome::BlockCreated(id) => id,
        other => panic!("expected a new block, got {other:?}"),
    }
}

fn p(page: &mut PageDoc, parent: Option<BlockId>) -> BlockId {
    add(page, parent, BlockKind::Paragraph)
}

fn indent(page: &mut PageDoc, ids: &[BlockId]) -> PageOutcome {
    page.apply(PageCommand::Indent { ids: ids.to_vec() }).unwrap()
}

fn outdent(page: &mut PageDoc, ids: &[BlockId]) -> PageOutcome {
    page.apply(PageCommand::Outdent { ids: ids.to_vec() }).unwrap()
}

fn kids(page: &PageDoc, id: BlockId) -> Vec<BlockId> {
    page.children(id).unwrap()
}

#[test]
fn indent_makes_a_block_the_last_child_of_the_block_above() {
    let mut page = PageDoc::new();
    let [a, b, c] = [(); 3].map(|()| p(&mut page, None));
    let a1 = p(&mut page, Some(a));

    assert_eq!(indent(&mut page, &[b]), PageOutcome::Changed);
    assert_eq!(page.root_blocks(), vec![a, c]);
    assert_eq!(kids(&page, a), vec![a1, b], "last child, after existing children");
    indent(&mut page, &[c]);
    assert_eq!(kids(&page, a), vec![a1, b, c]);
}

#[test]
fn indenting_several_blocks_keeps_them_together() {
    let mut page = PageDoc::new();
    let [a, b, c, d] = [(); 4].map(|()| p(&mut page, None));
    indent(&mut page, &[c, b]);
    assert_eq!(page.root_blocks(), vec![a, d]);
    assert_eq!(kids(&page, a), vec![b, c]);
}

#[test]
fn outdent_moves_a_block_after_its_parent_and_adopts_the_blocks_that_followed_it() {
    let mut page = PageDoc::new();
    let [parent, tail] = [(); 2].map(|()| p(&mut page, None));
    let [before, moved, after1, after2] = [(); 4].map(|()| p(&mut page, Some(parent)));
    let own_child = p(&mut page, Some(moved));

    assert_eq!(outdent(&mut page, &[moved]), PageOutcome::Changed);
    assert_eq!(page.root_blocks(), vec![parent, moved, tail]);
    assert_eq!(kids(&page, parent), vec![before]);
    assert_eq!(
        kids(&page, moved),
        vec![own_child, after1, after2],
        "followers go after its own children, in order"
    );
}

#[test]
fn outdenting_several_blocks_keeps_them_together() {
    let mut page = PageDoc::new();
    let a = p(&mut page, None);
    let [b, c, d] = [(); 3].map(|()| p(&mut page, Some(a)));
    outdent(&mut page, &[b, c]);
    assert_eq!(page.root_blocks(), vec![a, b, c]);
    assert_eq!(kids(&page, a), Vec::<BlockId>::new());
    assert_eq!(kids(&page, b), Vec::<BlockId>::new());
    assert_eq!(kids(&page, c), vec![d]);
}

#[test]
fn indent_then_outdent_of_a_last_child_round_trips() {
    let mut page = PageDoc::new();
    let [a, b] = [(); 2].map(|()| p(&mut page, None));
    indent(&mut page, &[b]);
    outdent(&mut page, &[b]);
    assert_eq!(page.root_blocks(), vec![a, b]);
    assert_eq!(kids(&page, a), Vec::<BlockId>::new());
}

#[test]
fn blocks_that_cannot_move_are_left_alone_without_writing() {
    let mut page = PageDoc::new();
    let first = p(&mut page, None);
    let heading = add(&mut page, None, BlockKind::Heading { level: 1 });
    let under_heading = p(&mut page, None);
    let parent = p(&mut page, None);
    let leaf_with_followers = add(&mut page, Some(parent), BlockKind::Divider);
    let follower = p(&mut page, Some(parent));

    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    let _sub = page.doc.subscribe_local_update(Box::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        true
    }));

    assert_eq!(
        indent(&mut page, &[first]),
        PageOutcome::Unchanged,
        "nothing above it"
    );
    assert_eq!(
        indent(&mut page, &[under_heading]),
        PageOutcome::Unchanged,
        "a heading can't hold children"
    );
    assert_eq!(
        outdent(&mut page, &[first]),
        PageOutcome::Unchanged,
        "already at the top level"
    );
    assert_eq!(
        outdent(&mut page, &[leaf_with_followers]),
        PageOutcome::Unchanged,
        "a divider can't adopt the block after it, so moving it would reorder the text"
    );
    assert_eq!(indent(&mut page, &[]), PageOutcome::Unchanged);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(kids(&page, parent), vec![leaf_with_followers, follower]);
    let _ = heading;

    assert_eq!(
        outdent(&mut page, &[follower]),
        PageOutcome::Changed,
        "a last child can always come out"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
