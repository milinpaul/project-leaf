//! Compaction (E03-S05): past 200 saved updates, a save folds them into one fresh snapshot, in one
//! transaction, without changing the document; an interrupted compaction or an unreadable update
//! leaves everything as it was.
#![allow(clippy::panic, reason = "test helpers fail a test by panicking")]

mod common;

use std::path::Path;

use common::TempDir;
use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Mark, PageCommand, PageDoc, PageOutcome, TextSpan, Workspace, WorkspaceCommand};
use leaf_store::{COMPACT_AFTER, DocKey, Workspace as Folder};
use rusqlite::{Connection, params};

/// `unwrap` for the helpers below, which clippy only allows in `#[test]` functions.
trait OrFail<T> {
    fn or_fail(self) -> T;
}

impl<T, E: std::fmt::Debug> OrFail<T> for Result<T, E> {
    fn or_fail(self) -> T {
        match self {
            Ok(value) => value,
            Err(err) => panic!("{err:?}"),
        }
    }
}

fn db(path: &Path) -> Connection {
    Connection::open(path.join("workspace.sqlite")).or_fail()
}

fn update_rows(path: &Path, key: DocKey) -> usize {
    let count: i64 = db(path)
        .query_row(
            "SELECT count(*) FROM doc_updates WHERE doc_id = ?1",
            [key.to_string()],
            |row| row.get(0),
        )
        .or_fail();
    usize::try_from(count).or_fail()
}

fn snapshot(path: &Path, key: DocKey) -> Vec<u8> {
    db(path)
        .query_row(
            "SELECT snapshot FROM docs WHERE doc_id = ?1",
            [key.to_string()],
            |row| row.get(0),
        )
        .or_fail()
}

fn texts(page: &PageDoc) -> Vec<(BlockId, Option<Vec<TextSpan>>)> {
    page.root_blocks()
        .into_iter()
        .map(|b| (b, page.text(b).or_fail()))
        .collect()
}

/// A page saved once as a snapshot, then once per typed letter: `saves` updates in all. The first
/// word is bold, so formatting has to survive too.
fn typed_page(folder: &mut Folder, key: DocKey, saves: usize) -> PageDoc {
    let mut page = PageDoc::new();
    folder.store_mut().save(key, &page, 0).or_fail();
    let PageOutcome::BlockCreated(block) = page
        .apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Paragraph,
            text: "bold ".into(),
        })
        .or_fail()
    else {
        panic!("expected a block");
    };
    page.apply(PageCommand::SetMark {
        id: block,
        range: 0..4,
        mark: Mark::Bold,
    })
    .or_fail();
    folder.store_mut().save(key, &page, 1).or_fail();
    for n in 1..saves {
        let pos = page
            .plain_text(block)
            .ok()
            .flatten()
            .map_or(0, |t| t.chars().count());
        page.apply(PageCommand::InsertText {
            id: block,
            pos,
            text: "x".into(),
        })
        .or_fail();
        folder
            .store_mut()
            .save(key, &page, 1 + i64::try_from(n).or_fail())
            .or_fail();
    }
    page
}

#[test]
fn the_save_past_the_limit_folds_the_updates_into_the_snapshot() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());

    let page = typed_page(&mut folder, key, COMPACT_AFTER);
    assert_eq!(update_rows(&path, key), COMPACT_AFTER, "at the limit: kept");
    let before = folder.store().saved_versions().unwrap()[&key];
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(texts(&loaded), texts(&page));

    let mut page = page;
    let block = page.root_blocks()[0];
    page.apply(PageCommand::InsertText {
        id: block,
        pos: 0,
        text: "!".into(),
    })
    .unwrap();
    assert!(folder.store_mut().save(key, &page, 500).unwrap());
    assert_eq!(update_rows(&path, key), 0, "folded away");
    let after = folder.store().saved_versions().unwrap()[&key];
    assert_eq!(after.saved_at, 500, "last saved: the last edit");
    assert_ne!(after, before);

    drop(folder);
    let folder = Folder::open(&path).unwrap();
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(texts(&loaded), texts(&page), "the same page, formatting included");
    assert_eq!(loaded.check_invariants(), Ok(()));
}

#[test]
fn saving_goes_on_as_before_after_compaction() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    let mut page = typed_page(&mut folder, key, COMPACT_AFTER + 1);
    assert_eq!(update_rows(&path, key), 0);

    let block = page.root_blocks()[0];
    page.apply(PageCommand::InsertText {
        id: block,
        pos: 0,
        text: "more ".into(),
    })
    .unwrap();
    assert!(folder.store_mut().save(key, &page, 1_000).unwrap());
    assert!(
        !folder.store_mut().save(key, &page, 1_001).unwrap(),
        "nothing new"
    );
    assert_eq!(update_rows(&path, key), 1, "one small update again");
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(texts(&loaded), texts(&page));
    assert_eq!(folder.store().saved_versions().unwrap()[&key].saved_at, 1_000);
}

#[test]
fn compacting_by_hand_does_nothing_below_the_limit_or_for_a_missing_document() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    typed_page(&mut folder, key, 3);
    assert!(!folder.store_mut().compact(key).unwrap());
    assert_eq!(update_rows(&path, key), 3);
    assert!(!folder.store_mut().compact(DocKey::Page(PageId::new())).unwrap());
}

#[test]
fn an_interrupted_compaction_leaves_the_snapshot_and_every_update() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    let mut page = typed_page(&mut folder, key, COMPACT_AFTER);
    let old_snapshot = snapshot(&path, key);
    // Fail the step after the new snapshot is written, as a crash or a full disk would.
    db(&path)
        .execute_batch(
            "CREATE TRIGGER interrupt BEFORE DELETE ON doc_updates
             BEGIN SELECT RAISE(ABORT, 'interrupted'); END;",
        )
        .unwrap();

    let block = page.root_blocks()[0];
    page.apply(PageCommand::InsertText {
        id: block,
        pos: 0,
        text: "!".into(),
    })
    .unwrap();
    assert!(folder.store_mut().compact(key).is_ok(), "not over the limit yet");
    assert!(
        folder.store_mut().save(key, &page, 500).unwrap(),
        "the save itself still succeeds"
    );
    assert!(folder.store_mut().compact(key).is_err());
    assert_eq!(snapshot(&path, key), old_snapshot, "rolled back");
    assert_eq!(update_rows(&path, key), COMPACT_AFTER + 1);
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(texts(&loaded), texts(&page), "nothing lost");

    db(&path).execute_batch("DROP TRIGGER interrupt;").unwrap();
    assert!(folder.store_mut().compact(key).unwrap(), "tried again later");
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(texts(&loaded), texts(&page));
}

#[test]
fn a_damaged_update_stops_compaction_so_it_is_never_folded_away() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    let mut page = typed_page(&mut folder, key, COMPACT_AFTER);
    db(&path)
        .execute(
            "UPDATE doc_updates SET data = x'00ff' WHERE seq = (SELECT min(seq) FROM doc_updates WHERE doc_id = ?1)",
            params![key.to_string()],
        )
        .unwrap();

    let block = page.root_blocks()[0];
    page.apply(PageCommand::InsertText {
        id: block,
        pos: 0,
        text: "!".into(),
    })
    .unwrap();
    assert!(folder.store_mut().save(key, &page, 500).unwrap());
    assert!(!folder.store_mut().compact(key).unwrap());
    assert_eq!(
        update_rows(&path, key),
        COMPACT_AFTER + 1,
        "the damaged row is kept"
    );
}

#[test]
fn an_update_missing_its_earlier_changes_stops_compaction() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let key = DocKey::Page(PageId::new());
    typed_page(&mut folder, key, COMPACT_AFTER + 1);
    let mut page: PageDoc = folder.store().load(key).unwrap().unwrap();
    let block = page.root_blocks()[0];
    for n in 0..=COMPACT_AFTER + 1 {
        page.apply(PageCommand::InsertText {
            id: block,
            pos: 0,
            text: "y".into(),
        })
        .unwrap();
        let wrote = folder
            .store_mut()
            .save(key, &page, 1_000 + i64::try_from(n).unwrap());
        assert!(wrote.unwrap());
        if n == 0 {
            // Lose the first update after the snapshot: everything after it waits for it.
            db(&path)
                .execute("DELETE FROM doc_updates WHERE doc_id = ?1", [key.to_string()])
                .unwrap();
        }
    }
    assert_eq!(
        update_rows(&path, key),
        COMPACT_AFTER + 1,
        "over the limit, but kept"
    );
    assert!(!folder.store_mut().compact(key).unwrap());
}

#[test]
fn the_workspace_document_is_compacted_too() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path).unwrap();
    let mut ws = Workspace::new();
    folder.store_mut().save(DocKey::Workspace, &ws, 0).unwrap();
    for n in 0..=COMPACT_AFTER {
        ws.apply(WorkspaceCommand::CreatePage {
            parent: None,
            index: 0,
            title: format!("page {n}"),
            created_at: 7,
        })
        .unwrap();
        let now = 1 + i64::try_from(n).unwrap();
        folder.store_mut().save(DocKey::Workspace, &ws, now).unwrap();
    }
    assert_eq!(update_rows(&path, DocKey::Workspace), 0);
    let loaded: Workspace = folder.store().load(DocKey::Workspace).unwrap().unwrap();
    assert_eq!(loaded.root_pages().unwrap(), ws.root_pages().unwrap());
}
