//! Autosave against a real workspace folder, with a fake clock (E03-S04).

mod common;

use common::TempDir;
use leaf_core::PageId;
use leaf_model::{BlockKind, PageCommand, PageDoc, Workspace, WorkspaceCommand};
use leaf_store::{Autosave, DocKey, Savable, Workspace as Folder};
use rusqlite::Connection;

fn rows(folder: &Folder) -> i64 {
    Connection::open(folder.root().join("workspace.sqlite"))
        .and_then(|c| {
            c.query_row(
                "SELECT (SELECT COUNT(*) FROM docs) + (SELECT COUNT(*) FROM doc_updates)",
                [],
                |row| row.get(0),
            )
        })
        .unwrap_or(-1)
}

fn type_into(page: &mut PageDoc, text: &str) {
    let result = page.apply(PageCommand::InsertBlock {
        parent: None,
        index: 0,
        kind: BlockKind::Paragraph,
        text: text.into(),
    });
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn due_documents_are_saved_once_and_then_nothing_is_written() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut autosave = Autosave::new(300);
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();

    for (i, now) in [0, 100, 250].into_iter().enumerate() {
        type_into(&mut page, &format!("line {i}"));
        autosave.changed(key, now);
    }
    // The timer fires at the deadline; before it nothing is due, so nothing is written.
    assert_eq!(autosave.due(299), Vec::<DocKey>::new());
    assert_eq!(rows(&folder), 0);
    let now = autosave.next_deadline().unwrap();
    for due in autosave.due(now) {
        assert!(autosave.save(folder.store_mut(), due, &page, now).unwrap());
    }
    assert_eq!(rows(&folder), 1, "three edits, one write");
    assert_eq!(autosave.next_deadline(), None);

    // Saving again with nothing changed writes nothing.
    assert!(!autosave.save(folder.store_mut(), key, &page, 400).unwrap());
    assert_eq!(rows(&folder), 1);

    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(loaded.root_blocks().len(), 3);
}

#[test]
fn quitting_flushes_everything_pending_even_if_not_due() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let mut autosave = Autosave::default();
    let mut ws = Workspace::new();
    let created = ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "Plans".into(),
        created_at: 10,
    });
    assert!(created.is_ok());
    autosave.changed(DocKey::Workspace, 10);

    // Quit 20 ms later, long before the deadline.
    for key in autosave.pending() {
        autosave.save(folder.store_mut(), key, &ws, 30).unwrap();
    }
    assert_eq!(autosave.pending(), Vec::<DocKey>::new());
    drop(folder);

    let folder = Folder::open(&path).unwrap();
    let loaded: Workspace = folder.store().load(DocKey::Workspace).unwrap().unwrap();
    let titles: Vec<String> = loaded
        .root_pages()
        .unwrap()
        .into_iter()
        .map(|id| loaded.page(id).unwrap().title)
        .collect();
    assert_eq!(titles, vec!["Plans"]);
}

#[test]
fn a_failed_save_stays_pending() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut autosave = Autosave::default();
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();
    type_into(&mut page, "keep me");
    autosave.changed(key, 0);

    // Make every write fail: the database refuses writes from now on.
    Connection::open(folder.root().join("workspace.sqlite"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER no_docs BEFORE INSERT ON docs BEGIN SELECT RAISE(ABORT, 'disk says no'); END;",
        )
        .unwrap();
    assert!(autosave.save(folder.store_mut(), key, &page, 300).is_err());
    assert_eq!(
        autosave.pending(),
        vec![key],
        "still unsaved, so retried next time"
    );
    assert!(page.unsaved_changes().unwrap().is_some());
}

#[test]
fn the_timer_notices_changes_itself_and_saves_them_when_due() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut autosave = Autosave::new(300);
    let ws = Workspace::new();
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();

    // Nothing changed: ticks write nothing and set no timer.
    let docs: [(DocKey, &dyn Savable); 2] = [(DocKey::Workspace, &ws), (key, &page)];
    assert_eq!(autosave.tick(folder.store_mut(), &docs, 0).unwrap(), 0);
    assert_eq!(autosave.next_deadline(), None);

    type_into(&mut page, "hello");
    let docs: [(DocKey, &dyn Savable); 2] = [(DocKey::Workspace, &ws), (key, &page)];
    assert_eq!(
        autosave.tick(folder.store_mut(), &docs, 100).unwrap(),
        0,
        "noticed, not due"
    );
    assert_eq!(autosave.next_deadline(), Some(400));
    assert_eq!(autosave.tick(folder.store_mut(), &docs, 400).unwrap(), 1);
    assert_eq!(autosave.tick(folder.store_mut(), &docs, 800).unwrap(), 0);
    assert_eq!(
        rows(&folder),
        1,
        "only the page; the untouched workspace isn't written"
    );
}

#[test]
fn flush_saves_everything_changed_at_once_and_forgets_vanished_documents() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut autosave = Autosave::default();
    let mut ws = Workspace::new();
    let created = ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "t".into(),
        created_at: 0,
    });
    assert!(created.is_ok());
    let gone = DocKey::Page(PageId::new());
    autosave.changed(gone, 0);

    let docs: [(DocKey, &dyn Savable); 1] = [(DocKey::Workspace, &ws)];
    assert_eq!(autosave.flush(folder.store_mut(), &docs, 5).unwrap(), 1);
    assert_eq!(autosave.pending(), Vec::<DocKey>::new());
    assert!(!ws.has_unsaved_changes());
}
