//! Turning pages into index rows (E11-S02).

use leaf_core::{AssetId, BlockId, PageId};
use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, PageOutcome, Workspace, WorkspaceCommand};
use leaf_search::{BlockText, UNTITLED, page_text};

/// Unwraps a helper's result. Helpers aren't `#[test]` functions, so clippy's test allowance
/// for `unwrap` doesn't reach them.
fn ok<T>(result: Result<T, leaf_model::ModelError>) -> T {
    result.unwrap_or_else(|err| panic!("model: {err}"))
}

fn new_page(ws: &mut Workspace, parent: Option<PageId>, title: &str) -> PageId {
    let outcome = ok(ws.apply(WorkspaceCommand::CreatePage {
        parent,
        index: 0,
        title: title.to_owned(),
        created_at: 1,
    }));
    let Outcome::PageCreated(id) = outcome else {
        panic!("expected a new page, got {outcome:?}")
    };
    id
}

fn add(doc: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    let index = match parent {
        Some(parent) => ok(doc.children(parent)).len(),
        None => doc.root_blocks().len(),
    };
    let outcome = ok(doc.apply(PageCommand::InsertBlock {
        parent,
        index,
        kind,
        text: text.to_owned(),
    }));
    let PageOutcome::BlockCreated(id) = outcome else {
        panic!("expected a new block, got {outcome:?}")
    };
    id
}

/// The `(kind, text)` of each row, which is what most tests care about.
fn rows(blocks: &[BlockText]) -> Vec<(&str, &str)> {
    blocks
        .iter()
        .map(|b| (b.kind.as_str(), b.text.as_str()))
        .collect()
}

#[test]
fn a_row_per_block_with_text_in_document_order() {
    let mut ws = Workspace::new();
    let page = new_page(&mut ws, None, "Trip plan");
    let mut doc = PageDoc::new();
    let intro = add(&mut doc, None, BlockKind::Heading { level: 1 }, "Lisbon");
    let list = add(&mut doc, None, BlockKind::BulletedList, "Pack");
    let nested = add(
        &mut doc,
        Some(list),
        BlockKind::Todo { checked: false },
        "Passport",
    );
    let after = add(&mut doc, None, BlockKind::Quote, "Travel light");

    let text = page_text(&ws, page, &doc).unwrap().unwrap();
    assert_eq!(text.page, page);
    assert_eq!(text.title, "Trip plan");
    assert_eq!(
        rows(&text.blocks),
        [
            ("heading", "Lisbon"),
            ("bulleted_list", "Pack"),
            ("todo", "Passport"),
            ("quote", "Travel light")
        ]
    );
    let ids: Vec<_> = text.blocks.iter().map(|b| b.block).collect();
    assert_eq!(ids, [intro, list, nested, after]);
}

#[test]
fn marks_are_left_out_of_the_text() {
    let mut ws = Workspace::new();
    let page = new_page(&mut ws, None, "Marks");
    let mut doc = PageDoc::new();
    let block = add(&mut doc, None, BlockKind::Paragraph, "bold words");
    doc.apply(PageCommand::SetMark {
        id: block,
        range: 0..4,
        mark: leaf_model::Mark::Bold,
    })
    .unwrap();
    assert_eq!(
        doc.text(block).unwrap().unwrap().len(),
        2,
        "the bold run is really there"
    );

    let text = page_text(&ws, page, &doc).unwrap().unwrap();
    assert_eq!(rows(&text.blocks), [("paragraph", "bold words")]);
}

#[test]
fn an_untitled_page_is_indexed_as_untitled() {
    let mut ws = Workspace::new();
    let blank = new_page(&mut ws, None, "");
    let spaces = new_page(&mut ws, None, "   ");
    let doc = PageDoc::new();

    assert_eq!(page_text(&ws, blank, &doc).unwrap().unwrap().title, UNTITLED);
    assert_eq!(page_text(&ws, spaces, &doc).unwrap().unwrap().title, UNTITLED);
}

#[test]
fn trashed_pages_and_pages_under_them_give_nothing() {
    let mut ws = Workspace::new();
    let parent = new_page(&mut ws, None, "Old");
    let child = new_page(&mut ws, Some(parent), "Older");
    let doc = PageDoc::new();
    ws.apply(WorkspaceCommand::TrashPage { page: parent, at: 2 })
        .unwrap();

    assert_eq!(page_text(&ws, parent, &doc).unwrap(), None);
    assert_eq!(page_text(&ws, child, &doc).unwrap(), None);

    ws.apply(WorkspaceCommand::RestorePage { page: parent }).unwrap();
    assert!(page_text(&ws, child, &doc).unwrap().is_some());
}

#[test]
fn dividers_and_empty_blocks_give_no_row_but_code_and_captions_do() {
    let mut ws = Workspace::new();
    let page = new_page(&mut ws, None, "Kinds");
    let mut doc = PageDoc::new();
    add(&mut doc, None, BlockKind::Divider, "");
    add(&mut doc, None, BlockKind::Paragraph, "");
    add(&mut doc, None, BlockKind::Paragraph, "  \n ");
    add(
        &mut doc,
        None,
        BlockKind::Code {
            language: "rust".to_owned(),
        },
        "fn main() {}",
    );
    let image = BlockKind::Image {
        asset: AssetId::of(b"png"),
        width: None,
        alt: String::new(),
    };
    add(&mut doc, None, image, "Sunset over the bay");

    let text = page_text(&ws, page, &doc).unwrap().unwrap();
    assert_eq!(
        rows(&text.blocks),
        [("code", "fn main() {}"), ("image", "Sunset over the bay")]
    );
}

#[test]
fn unknown_kinds_are_indexed_by_their_text_and_left_untouched() {
    // A block from a newer Leaf, written raw the way that version would.
    let (with_text, without_text) = (BlockId::new(), BlockId::new());
    let raw = loro::LoroDoc::new();
    let tree = raw.get_tree("blocks");
    tree.enable_fractional_index(0);
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert("id", with_text.to_string()).unwrap();
    meta.insert("type", "callout").unwrap();
    meta.ensure_mergeable_text("text")
        .unwrap()
        .insert(0, "Remember the keys")
        .unwrap();
    let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
    meta.insert("id", without_text.to_string()).unwrap();
    meta.insert("type", "kanban").unwrap();
    raw.commit();
    let bytes = raw.export(loro::ExportMode::Snapshot).unwrap();
    let doc = PageDoc::from_bytes(&bytes).unwrap();

    let mut ws = Workspace::new();
    let page = new_page(&mut ws, None, "From the future");
    let text = page_text(&ws, page, &doc).unwrap().unwrap();

    assert_eq!(rows(&text.blocks), [("callout", "Remember the keys")]);
    assert_eq!(text.blocks[0].block, with_text);
    assert!(
        !doc.has_unsaved_changes(),
        "extracting text must not change the page"
    );
    assert_eq!(
        doc.to_bytes().unwrap(),
        PageDoc::from_bytes(&bytes).unwrap().to_bytes().unwrap()
    );
}

#[test]
fn a_page_missing_from_the_workspace_is_an_error() {
    let ws = Workspace::new();
    let err = page_text(&ws, PageId::new(), &PageDoc::new()).unwrap_err();
    assert!(matches!(err, leaf_search::SearchError::Model(_)));
    assert!(err.to_string().starts_with("search could not read a page"));
}
