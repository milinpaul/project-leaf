//! Saving and loading documents through a workspace folder (E03-S03).
#![allow(clippy::panic, reason = "test helpers fail a test by panicking")]

mod common;

use std::path::Path;

use common::TempDir;
use leaf_core::{BlockId, PageId};
use leaf_model::{
    BlockKind, Mark, Outcome, PageCommand, PageDoc, PageOutcome, TextSpan, Workspace, WorkspaceCommand,
};
use leaf_store::{DocKey, Workspace as Folder};
use rusqlite::Connection;

type Outline = Vec<(usize, BlockId, BlockKind, Option<Vec<TextSpan>>)>;

fn outline(page: &PageDoc) -> Outline {
    fn walk(page: &PageDoc, blocks: Vec<BlockId>, depth: usize, out: &mut Outline) {
        for block in blocks {
            out.push((
                depth,
                block,
                page.kind(block).ok().unwrap_or(BlockKind::Paragraph),
                page.text(block).ok().flatten(),
            ));
            walk(page, page.children(block).unwrap_or_default(), depth + 1, out);
        }
    }
    let mut out = Vec::new();
    walk(page, page.root_blocks(), 0, &mut out);
    out
}

fn sidebar(ws: &Workspace) -> Vec<(PageId, String, Option<String>, Vec<PageId>)> {
    let mut out = Vec::new();
    let mut queue = ws.root_pages().unwrap_or_default();
    while let Some(id) = queue.first().copied() {
        queue.remove(0);
        let children = ws.children(id).unwrap_or_default();
        if let Ok(meta) = ws.page(id) {
            out.push((id, meta.title, meta.icon, children.clone()));
        }
        queue.extend(children);
    }
    out
}

fn update_rows(path: &Path, doc: &str) -> i64 {
    Connection::open(path.join("workspace.sqlite"))
        .and_then(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM doc_updates WHERE doc_id = ?1",
                [doc],
                |row| row.get(0),
            )
        })
        .unwrap_or(-1)
}

fn new_page(ws: &mut Workspace, parent: Option<PageId>, title: &str) -> PageId {
    match ws.apply(WorkspaceCommand::CreatePage {
        parent,
        index: 0,
        title: title.into(),
        created_at: 7,
    }) {
        Ok(Outcome::PageCreated(id)) => id,
        other => panic!("expected a page, got {other:?}"),
    }
}

fn insert(page: &mut PageDoc, parent: Option<BlockId>, kind: BlockKind, text: &str) -> BlockId {
    match page.apply(PageCommand::InsertBlock {
        parent,
        index: 0,
        kind,
        text: text.into(),
    }) {
        Ok(PageOutcome::BlockCreated(id)) => id,
        other => panic!("expected a block, got {other:?}"),
    }
}

#[test]
fn the_workspace_survives_save_and_reopen() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let store = folder.store_mut();
    let mut ws = Workspace::new();
    ws.set_undo_merge_interval(0);

    let notes = new_page(&mut ws, None, "Notes");
    assert!(
        store.save(DocKey::Workspace, &ws, 1).unwrap(),
        "first save: snapshot"
    );
    new_page(&mut ws, Some(notes), "Inside");
    ws.apply(WorkspaceCommand::SetPageIcon {
        page: notes,
        icon: Some("🌿".into()),
    })
    .unwrap();
    assert!(store.save(DocKey::Workspace, &ws, 2).unwrap());
    new_page(&mut ws, None, "Gone again");
    assert!(ws.undo().unwrap());
    assert!(store.save(DocKey::Workspace, &ws, 3).unwrap());
    let expected = sidebar(&ws);
    drop(folder);

    let folder = Folder::open(&path).unwrap();
    let loaded: Workspace = folder.store().load(DocKey::Workspace).unwrap().unwrap();
    assert_eq!(sidebar(&loaded), expected);
    assert_eq!(expected.len(), 2);
    assert_eq!(
        update_rows(&path, "workspace"),
        2,
        "one row per save after the snapshot"
    );
}

#[test]
fn pages_survive_save_and_reopen_through_many_rounds() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();
    page.set_undo_merge_interval(0);

    folder.store_mut().save(key, &page, 0).unwrap();
    let list = insert(&mut page, None, BlockKind::BulletedList, "list");
    let child = insert(
        &mut page,
        Some(list),
        BlockKind::Todo { checked: true },
        "child 👋🏽",
    );
    folder.store_mut().save(key, &page, 1).unwrap();
    page.apply(PageCommand::SetMark {
        id: child,
        range: 0..5,
        mark: Mark::Bold,
    })
    .unwrap();
    page.apply(PageCommand::SplitBlock { id: list, pos: 2 }).unwrap();
    folder.store_mut().save(key, &page, 2).unwrap();
    page.undo().unwrap();
    folder.store_mut().save(key, &page, 3).unwrap();
    let expected = outline(&page);
    drop(folder);

    let folder = Folder::open(&path).unwrap();
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(outline(&loaded), expected);
    assert_eq!(loaded.check_invariants(), Ok(()));
    assert_eq!(update_rows(&path, &key.to_string()), 3);
}

#[test]
fn a_table_survives_save_and_reopen_unchanged() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();

    let PageOutcome::BlockCreated(table) = page
        .apply(PageCommand::InsertTable {
            parent: None,
            index: 0,
            rows: 2,
            columns: 2,
            header_row: true,
        })
        .unwrap()
    else {
        panic!("expected a table");
    };
    folder.store_mut().save(key, &page, 0).unwrap();
    let cell = page.table_cells(table).unwrap()[1][0];
    page.apply(PageCommand::InsertText {
        id: cell,
        pos: 0,
        text: "Tomatoes".into(),
    })
    .unwrap();
    page.apply(PageCommand::SetMark {
        id: cell,
        range: 0..3,
        mark: Mark::Italic,
    })
    .unwrap();
    page.apply(PageCommand::InsertTableColumn { table, index: 1 })
        .unwrap();
    folder.store_mut().save(key, &page, 1).unwrap();
    let expected = (outline(&page), page.table_cells(table).unwrap());
    drop(folder);

    let folder = Folder::open(&path).unwrap();
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!((outline(&loaded), loaded.table_cells(table).unwrap()), expected);
    assert_eq!(loaded.kind(table).unwrap(), BlockKind::Table { header_row: true });
    assert_eq!(loaded.check_invariants(), Ok(()));
}

#[test]
fn saving_with_nothing_changed_writes_nothing() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let mut ws = Workspace::new();
    assert!(folder.store_mut().save(DocKey::Workspace, &ws, 1).unwrap());
    assert!(!folder.store_mut().save(DocKey::Workspace, &ws, 2).unwrap());

    // A command that changes nothing doesn't count either.
    let page = new_page(&mut ws, None, "same");
    folder.store_mut().save(DocKey::Workspace, &ws, 3).unwrap();
    ws.apply(WorkspaceCommand::RenamePage {
        page,
        title: "same".into(),
    })
    .unwrap();
    assert!(!folder.store_mut().save(DocKey::Workspace, &ws, 4).unwrap());
    assert_eq!(update_rows(&path, "workspace"), 1);
}

#[test]
fn documents_are_kept_apart_and_unsaved_ones_load_as_none() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let (a, b) = (DocKey::Page(PageId::new()), DocKey::Page(PageId::new()));
    let mut first = PageDoc::new();
    insert(&mut first, None, BlockKind::Paragraph, "a");
    let mut second = PageDoc::new();
    insert(&mut second, None, BlockKind::Quote, "b");
    let store = folder.store_mut();
    store.save(a, &first, 1).unwrap();
    store.save(b, &second, 1).unwrap();

    let loaded: PageDoc = store.load(a).unwrap().unwrap();
    assert_eq!(outline(&loaded), outline(&first));
    let loaded: PageDoc = store.load(b).unwrap().unwrap();
    assert_eq!(outline(&loaded), outline(&second));
    assert!(
        store
            .load::<PageDoc>(DocKey::Page(PageId::new()))
            .unwrap()
            .is_none()
    );
    assert!(store.load::<Workspace>(DocKey::Workspace).unwrap().is_none());
}

#[test]
fn keys_are_stored_as_the_spec_says() {
    let page = PageId::new();
    assert_eq!(DocKey::Workspace.to_string(), "workspace");
    assert_eq!(DocKey::Page(page).to_string(), page.to_string());
}

#[test]
fn deleting_a_page_document_removes_its_snapshot_and_updates_only() {
    let dir = TempDir::new();
    let path = dir.0.join("W.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let (gone, kept) = (PageId::new(), PageId::new());
    for (page, text) in [(gone, "bye"), (kept, "stay")] {
        let mut doc = PageDoc::new();
        insert(&mut doc, None, BlockKind::Paragraph, text);
        folder.store_mut().save(DocKey::Page(page), &doc, 1).unwrap();
        insert(&mut doc, None, BlockKind::Paragraph, "more"); // an update row too
        folder.store_mut().save(DocKey::Page(page), &doc, 2).unwrap();
    }
    assert!(folder.store_mut().delete(DocKey::Page(gone)).unwrap());
    assert!(
        folder
            .store()
            .load::<PageDoc>(DocKey::Page(gone))
            .unwrap()
            .is_none()
    );
    assert_eq!(update_rows(&path, &gone.to_string()), 0);
    assert_eq!(update_rows(&path, &kept.to_string()), 1);
    let left: PageDoc = folder.store().load(DocKey::Page(kept)).unwrap().unwrap();
    assert_eq!(left.root_blocks().len(), 2, "the other page is untouched");
    assert!(
        !folder.store_mut().delete(DocKey::Page(gone)).unwrap(),
        "nothing left to remove"
    );
}
