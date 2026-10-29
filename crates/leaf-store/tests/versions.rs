//! Saved versions, which search compares to decide what to re-index (E11-S04).

mod common;

use common::TempDir;
use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, Workspace, WorkspaceCommand};
use leaf_store::{DocKey, SavedVersion, Workspace as Folder};
use rusqlite::Connection;

fn create_page(ws: &mut Workspace) -> leaf_core::PageId {
    match ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "Notes".into(),
        created_at: 1,
    }) {
        Ok(Outcome::PageCreated(id)) => id,
        other => panic!("expected a new page, got {other:?}"),
    }
}

fn type_into(doc: &mut PageDoc, text: &str) {
    if let Err(err) = doc.apply(PageCommand::InsertBlock {
        parent: None,
        index: doc.root_blocks().len(),
        kind: BlockKind::Paragraph,
        text: text.into(),
    }) {
        panic!("insert failed: {err}");
    }
}

#[test]
fn every_save_changes_the_version_even_within_one_millisecond() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut ws = Workspace::new();
    let page = create_page(&mut ws);
    let mut doc = PageDoc::new();
    assert!(folder.store().saved_versions().unwrap().is_empty());

    folder.store_mut().save(DocKey::Workspace, &ws, 10).unwrap();
    type_into(&mut doc, "first");
    folder.store_mut().save(DocKey::Page(page), &doc, 10).unwrap();
    let first = folder.store().saved_versions().unwrap();
    assert_eq!(
        first[&DocKey::Page(page)],
        SavedVersion { saved_at: 10, seq: 0 },
        "a snapshot alone"
    );
    assert_eq!(first[&DocKey::Workspace].saved_at, 10);

    type_into(&mut doc, "second");
    folder.store_mut().save(DocKey::Page(page), &doc, 10).unwrap();
    let second = folder.store().saved_versions().unwrap()[&DocKey::Page(page)];
    assert_eq!(second.saved_at, 10);
    assert!(second.seq > 0);

    type_into(&mut doc, "third");
    folder.store_mut().save(DocKey::Page(page), &doc, 25).unwrap();
    let third = folder.store().saved_versions().unwrap()[&DocKey::Page(page)];
    assert!(third.seq > second.seq);
    assert_eq!(third.saved_at, 25);
    assert_eq!(
        folder.store().saved_versions().unwrap()[&DocKey::Workspace],
        first[&DocKey::Workspace]
    );
}

#[test]
fn rows_with_unknown_ids_are_skipped() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    folder
        .store_mut()
        .save(DocKey::Workspace, &Workspace::new(), 5)
        .unwrap();
    Connection::open(path.join("workspace.sqlite"))
        .unwrap()
        .execute(
            "INSERT INTO docs (doc_id, snapshot, updated_at) VALUES ('not-a-page', x'00', 1)",
            [],
        )
        .unwrap();

    let versions = folder.store().saved_versions().unwrap();
    assert_eq!(versions.len(), 1);
    assert!(versions.contains_key(&DocKey::Workspace));
}
