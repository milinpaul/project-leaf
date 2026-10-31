//! Backlinks (E10-S06): which blocks link to a page, read from the index that page text fills.

use std::path::PathBuf;

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, PageOutcome, Workspace, WorkspaceCommand};
use leaf_search::{Backlink, LINK_CONTEXT, PageVersion, SearchIndex, Searcher, page_text};

/// Unwraps a helper's result. Helpers aren't `#[test]` functions, so clippy's test allowance for
/// `unwrap` doesn't reach them.
fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|err| panic!("{err}"))
}

fn new_page(ws: &mut Workspace, title: &str) -> PageId {
    let index = ok(ws.child_slots(None)).len();
    let Outcome::PageCreated(id) = ok(ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index,
        title: title.to_owned(),
        created_at: 1,
    })) else {
        panic!("a page")
    };
    id
}

/// A block of `text` where each `@` is a link to the next of `targets`.
fn add(doc: &mut PageDoc, text: &str, targets: &[PageId]) -> BlockId {
    let index = doc.root_blocks().len();
    let PageOutcome::BlockCreated(id) = ok(doc.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind: BlockKind::Paragraph,
        text: text.replace('@', ""),
    })) else {
        panic!("a block")
    };
    let spots = text
        .chars()
        .enumerate()
        .filter(|(_, c)| *c == '@')
        .map(|(at, _)| at);
    for (pos, page) in spots.zip(targets) {
        ok(doc.apply(PageCommand::InsertPageLink { id, pos, page: *page }));
    }
    id
}

/// A fresh index in its own folder.
fn index(name: &str) -> (SearchIndex, PathBuf) {
    let root = std::env::temp_dir().join(format!("leaf-backlinks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    ok(std::fs::create_dir_all(&root));
    let (index, _) = ok(SearchIndex::open(&root));
    (index, root)
}

fn put(index: &mut SearchIndex, ws: &Workspace, page: PageId, doc: &PageDoc) {
    match ok(page_text(ws, page, doc)) {
        Some(text) => ok(index.index_page(&text, PageVersion::default())),
        None => ok(index.remove_page(page)),
    }
}

fn summary(links: &[Backlink]) -> Vec<(String, String, String)> {
    links
        .iter()
        .map(|l| (l.title.clone(), l.before.clone(), l.after.clone()))
        .collect()
}

#[test]
fn backlinks_list_each_linking_block_with_the_words_around_it() {
    let mut ws = Workspace::new();
    let japan = new_page(&mut ws, "Trip to Japan");
    let (budget, notes, journal) = (
        new_page(&mut ws, "Budget"),
        new_page(&mut ws, "notes"),
        new_page(&mut ws, "Journal"),
    );
    let (mut index, root) = index("list");

    let mut budget_doc = PageDoc::new();
    let first = add(&mut budget_doc, "Flights for @ are booked", &[japan]);
    // Two links to the same page in one block: one entry. A link elsewhere doesn't count.
    let second = add(&mut budget_doc, "@ again and @ and @", &[japan, japan, journal]);
    put(&mut index, &ws, budget, &budget_doc);
    let mut notes_doc = PageDoc::new();
    let note = add(&mut notes_doc, "@", &[japan]);
    put(&mut index, &ws, notes, &notes_doc);
    // A page linking to itself is left out.
    let mut japan_doc = PageDoc::new();
    add(&mut japan_doc, "see @", &[japan]);
    put(&mut index, &ws, japan, &japan_doc);

    let links = ok(index.backlinks(japan));
    assert_eq!(
        summary(&links),
        vec![
            ("Budget".into(), "Flights for ".into(), " are booked".into()),
            ("Budget".into(), String::new(), " again and  and ".into()),
            ("notes".into(), String::new(), String::new()),
        ],
        "pages by title (case ignored), blocks in reading order"
    );
    assert_eq!(
        links.iter().map(|l| (l.page, l.block)).collect::<Vec<_>>(),
        vec![(budget, first), (budget, second), (notes, note)]
    );
    // The search bar's read-only connection sees the same.
    let searcher = ok(Searcher::open(&root));
    assert_eq!(ok(searcher.backlinks(japan)), links);
    assert_eq!(
        summary(&ok(searcher.backlinks(journal))),
        vec![("Budget".into(), " again and  and ".into(), String::new())]
    );
    assert_eq!(ok(searcher.backlinks(PageId::new())), Vec::new());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn long_text_around_a_link_is_cut_with_an_ellipsis() {
    let mut ws = Workspace::new();
    let (target, source) = (new_page(&mut ws, "Target"), new_page(&mut ws, "Source"));
    let (mut index, root) = index("cut");
    let long = "word ".repeat(30);
    let mut doc = PageDoc::new();
    add(&mut doc, &format!("{long}@{long}"), &[target]);
    put(&mut index, &ws, source, &doc);
    let link = ok(index.backlinks(target)).remove(0);
    assert!(link.before.starts_with('…') && link.before.chars().count() <= LINK_CONTEXT + 1);
    assert!(link.after.ends_with('…') && link.after.chars().count() <= LINK_CONTEXT + 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn backlinks_follow_edits_trash_and_removal() {
    let mut ws = Workspace::new();
    let (target, source) = (new_page(&mut ws, "Target"), new_page(&mut ws, "Source"));
    let (mut index, root) = index("follow");
    let mut doc = PageDoc::new();
    let block = add(&mut doc, "a @ b", &[target]);
    put(&mut index, &ws, source, &doc);
    assert_eq!(ok(index.backlinks(target)).len(), 1);

    // The link deleted and the page indexed again: gone.
    ok(doc.apply(PageCommand::DeleteText {
        id: block,
        range: 2..3,
    }));
    put(&mut index, &ws, source, &doc);
    assert_eq!(ok(index.backlinks(target)), Vec::new());

    // Back, then the linking page trashed: page text is `None`, so it leaves the index with its links.
    add(&mut doc, "@", &[target]);
    put(&mut index, &ws, source, &doc);
    assert_eq!(ok(index.backlinks(target)).len(), 1);
    ok(ws.apply(WorkspaceCommand::TrashPage { page: source, at: 2 }));
    put(&mut index, &ws, source, &doc);
    assert_eq!(ok(index.backlinks(target)), Vec::new());

    // Restored, then the index emptied for a rebuild.
    ok(ws.apply(WorkspaceCommand::RestorePage { page: source }));
    put(&mut index, &ws, source, &doc);
    assert_eq!(ok(index.backlinks(target)).len(), 1);
    ok(index.clear());
    assert_eq!(ok(index.backlinks(target)), Vec::new());
    let _ = std::fs::remove_dir_all(root);
}
