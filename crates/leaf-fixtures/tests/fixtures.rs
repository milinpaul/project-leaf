//! The fixtures: their sizes, that a seed fixes their outline, that they are valid documents, and
//! that they save and load like any workspace.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use leaf_core::{BlockId, PageId};
use leaf_fixtures::{
    BIG_PAGE_BLOCKS, BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE, big_page, big_workspace, page_tree, save_to_folder,
};
use leaf_model::{BlockKind, PageDoc, Workspace};
use leaf_store::DocKey;

/// A page's outline without its IDs: one line per block, indented by depth, with its kind, text and
/// formatting. Two pages with equal outlines differ only in IDs.
fn page_outline(page: &PageDoc) -> String {
    fn walk(page: &PageDoc, blocks: Vec<BlockId>, depth: usize, out: &mut String) {
        for block in blocks {
            let kind = page.kind(block).unwrap();
            let text = page.text(block).unwrap();
            writeln!(out, "{:depth$}{kind:?} {text:?}", "").unwrap();
            walk(page, page.children(block).unwrap(), depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(page, page.root_blocks(), 0, &mut out);
    out
}

/// A workspace's page tree without IDs: titles and icons, indented by depth.
fn tree_outline(workspace: &Workspace) -> String {
    fn walk(workspace: &Workspace, pages: Vec<PageId>, depth: usize, out: &mut String) {
        for page in pages {
            let meta = workspace.page(page).unwrap();
            writeln!(out, "{:depth$}{:?} {:?}", "", meta.title, meta.icon).unwrap();
            walk(workspace, workspace.children(page).unwrap(), depth + 1, out);
        }
    }
    let mut out = String::new();
    walk(workspace, workspace.root_pages().unwrap(), 0, &mut out);
    out
}

fn depth_of(page: &PageDoc, mut block: BlockId) -> usize {
    let mut depth = 0;
    while let Some(parent) = page.parent(block).unwrap() {
        depth += 1;
        block = parent;
    }
    depth
}

#[test]
fn the_same_seed_gives_the_same_page_and_another_seed_a_different_one() {
    let a = big_page(42, 300).unwrap();
    let b = big_page(42, 300).unwrap();
    let c = big_page(43, 300).unwrap();
    assert_eq!(page_outline(&a), page_outline(&b));
    assert_ne!(page_outline(&a), page_outline(&c));
}

#[test]
fn the_same_seed_gives_the_same_workspace() {
    let a = big_workspace(7, 60, 5).unwrap();
    let b = big_workspace(7, 60, 5).unwrap();
    assert_eq!(tree_outline(&a.workspace), tree_outline(&b.workspace));
    let outlines = |fixture: &leaf_fixtures::Fixture| -> Vec<String> {
        fixture.pages.iter().map(|(_, page)| page_outline(page)).collect()
    };
    assert_eq!(outlines(&a), outlines(&b));
    assert_ne!(
        tree_outline(&a.workspace),
        tree_outline(&big_workspace(8, 60, 5).unwrap().workspace)
    );
    assert_eq!(
        tree_outline(&page_tree(7, 60).unwrap()),
        tree_outline(&a.workspace),
        "the page tree doesn't depend on whether pages get blocks"
    );
}

#[test]
fn a_big_page_has_every_kind_nesting_and_formatting() {
    let page = big_page(1, 2_000).unwrap();
    assert_eq!(page.len(), 2_000);
    page.check_invariants().unwrap();
    let all = {
        let mut all = Vec::new();
        let mut stack = page.root_blocks();
        while let Some(block) = stack.pop() {
            stack.extend(page.children(block).unwrap());
            all.push(block);
        }
        all
    };
    let kinds: std::collections::BTreeSet<String> = all
        .iter()
        .map(|b| page.kind(*b).unwrap().type_name().to_owned())
        .collect();
    for kind in [
        "paragraph",
        "heading",
        "bulleted_list",
        "numbered_list",
        "todo",
        "quote",
        "code",
        "divider",
    ] {
        assert!(kinds.contains(kind), "no {kind} in {kinds:?}");
    }
    let deepest = all.iter().map(|b| depth_of(&page, *b)).max().unwrap();
    assert!(
        (1..=3).contains(&deepest),
        "lists nest, up to three levels: {deepest}"
    );
    let formatted = all
        .iter()
        .filter_map(|b| page.text(*b).unwrap())
        .filter(|spans| {
            spans
                .iter()
                .any(|span| span.marks != leaf_model::Marks::default())
        })
        .count();
    assert!(formatted > 100, "some text is formatted: {formatted}");
    assert!(
        all.iter()
            .any(|b| matches!(page.kind(*b).unwrap(), BlockKind::Todo { checked: true })),
        "some to-dos are ticked"
    );
}

#[test]
fn the_full_size_page_and_page_tree() {
    let started = Instant::now();
    let page = big_page(2026, BIG_PAGE_BLOCKS).unwrap();
    assert_eq!(page.len(), BIG_PAGE_BLOCKS);
    let tree = page_tree(2026, BIG_WORKSPACE_PAGES).unwrap();
    assert_eq!(tree.len(), BIG_WORKSPACE_PAGES);
    let deepest = (0..)
        .scan(tree.root_pages().unwrap(), |level, depth| {
            let next: Vec<PageId> = level.iter().flat_map(|p| tree.children(*p).unwrap()).collect();
            (!level.is_empty()).then(|| {
                *level = next;
                depth
            })
        })
        .last()
        .unwrap();
    assert!((3..=6).contains(&deepest), "a tree a few levels deep: {deepest}");
    assert!(
        tree.root_pages().unwrap().len() > 100,
        "and many top-level sections"
    );
    // Generous for a debug build on a busy machine; it takes a few seconds.
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn a_fixture_saves_to_a_folder_and_loads_back_the_same() {
    let dir = std::env::temp_dir().join(format!("leaf-fixtures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fixture = big_workspace(3, 40, BLOCKS_PER_PAGE).unwrap();
    assert_eq!(fixture.block_count(), 40 * BLOCKS_PER_PAGE);
    let folder = save_to_folder(&dir, &fixture).unwrap();
    let store = folder.store();
    let workspace: Workspace = store.load(DocKey::Workspace).unwrap().unwrap();
    assert_eq!(tree_outline(&workspace), tree_outline(&fixture.workspace));
    for (id, page) in &fixture.pages {
        let loaded: PageDoc = store.load(DocKey::Page(*id)).unwrap().unwrap();
        assert_eq!(page_outline(&loaded), page_outline(page));
    }
    drop(folder);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_full_size_workspace_for_search() {
    let started = Instant::now();
    let fixture = big_workspace(2026, BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE).unwrap();
    assert_eq!(fixture.workspace.len(), BIG_WORKSPACE_PAGES);
    assert_eq!(fixture.block_count(), 100_000, "the search budget's 5,000 pages and 100,000 blocks");
    eprintln!("built in {:?}", started.elapsed());
}
