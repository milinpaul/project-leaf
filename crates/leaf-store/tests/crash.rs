//! Crash recovery (E03-S06): what a killed app leaves on disk loads again, losing nothing older
//! than the autosave delay, and a damaged update is skipped instead of failing the load.
//!
//! A crash is simulated by copying the database and its write-ahead log while the app still has
//! them open: exactly the files a killed process leaves behind, with nothing flushed or closed.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::TempDir;
use leaf_core::PageId;
use leaf_model::{BlockKind, PageCommand, PageDoc, Workspace, WorkspaceCommand};
use leaf_store::{Autosave, DEFAULT_AUTOSAVE_MS, DocKey, StoreError, Workspace as Folder};
use proptest::prelude::*;
use rusqlite::Connection;

/// Copies the files on disk right now into a fresh workspace folder, as if the app had been killed.
fn crash_copy(folder: &Folder, to: &Path) -> std::io::Result<PathBuf> {
    let copy = to.join("after-crash.leaf");
    fs::create_dir_all(&copy)?;
    for name in ["workspace.sqlite", "workspace.sqlite-wal"] {
        let from = folder.root().join(name);
        if from.exists() {
            fs::copy(from, copy.join(name))?;
        }
    }
    Ok(copy)
}

fn texts(page: &PageDoc) -> Vec<String> {
    page.root_blocks()
        .into_iter()
        .filter_map(|b| page.plain_text(b).ok().flatten())
        .collect()
}

fn append(page: &mut PageDoc, text: String) -> Result<(), TestCaseError> {
    let index = page.root_blocks().len();
    page.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind: BlockKind::Paragraph,
        text,
    })
    .map_err(|e| TestCaseError::fail(e.to_string()))?;
    Ok(())
}

/// Runs the autosave timer up to `now`: saves at every deadline that has passed, at that deadline.
fn run_timer(
    autosave: &mut Autosave,
    folder: &mut Folder,
    page: &mut PageDoc,
    now: i64,
) -> Result<(), TestCaseError> {
    while let Some(deadline) = autosave.next_deadline().filter(|&d| d <= now) {
        for due in autosave.due(deadline) {
            autosave
                .save(folder.store_mut(), due, page, deadline)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Edits at random moments, the timer saving at each deadline, then a crash at a random moment
    /// with no flush. Every edit made at least the autosave delay before the crash survives.
    #[test]
    fn a_crash_loses_nothing_older_than_the_autosave_delay(
        gaps in proptest::collection::vec(0..400_i64, 1..40),
        crash_after in 0..1_000_i64,
    ) {
        let dir = TempDir::new();
        let mut folder = Folder::create(&dir.0.join("w.leaf")).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let key = DocKey::Page(PageId::new());
        let mut page = PageDoc::new();
        let mut autosave = Autosave::default();
        // The page exists on disk from the start, as when the app creates it.
        autosave.save(folder.store_mut(), key, &page, 0).map_err(|e| TestCaseError::fail(e.to_string()))?;

        let mut now = 0;
        let mut edits = Vec::new();
        for (i, gap) in gaps.into_iter().enumerate() {
            now += gap;
            run_timer(&mut autosave, &mut folder, &mut page, now)?;
            append(&mut page, format!("edit {i}"))?;
            autosave.changed(key, now);
            edits.push((now, format!("edit {i}")));
        }
        let crash = now + crash_after;
        run_timer(&mut autosave, &mut folder, &mut page, crash)?;

        let copy = crash_copy(&folder, &dir.0).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let reopened = Folder::open(&copy).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let loaded: PageDoc = reopened
            .store()
            .load(key)
            .map_err(|e| TestCaseError::fail(e.to_string()))?
            .ok_or_else(|| TestCaseError::fail("the page is gone"))?;
        let kept = texts(&loaded);
        for (at, text) in &edits {
            if at + DEFAULT_AUTOSAVE_MS <= crash {
                prop_assert!(kept.contains(text), "{text} at {at} lost in a crash at {crash}");
            }
        }
        // Whatever survived is an unbroken prefix of what was typed: no holes, no reordering.
        let typed: Vec<String> = edits.into_iter().map(|(_, t)| t).collect();
        prop_assert_eq!(&kept[..], &typed[..kept.len()]);
    }
}

#[test]
fn the_files_a_killed_app_leaves_behind_open_and_load() {
    let dir = TempDir::new();
    let mut folder = Folder::create(&dir.0.join("w.leaf")).unwrap();
    let mut ws = Workspace::new();
    ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 0,
        title: "Saved".into(),
        created_at: 1,
    })
    .unwrap();
    folder.store_mut().save(DocKey::Workspace, &ws, 1).unwrap();
    ws.apply(WorkspaceCommand::CreatePage {
        parent: None,
        index: 1,
        title: "Not yet saved".into(),
        created_at: 2,
    })
    .unwrap();

    let copy = crash_copy(&folder, &dir.0).unwrap();
    assert!(
        copy.join("workspace.sqlite-wal").exists(),
        "the save is still only in the write-ahead log"
    );
    let reopened = Folder::open(&copy).unwrap();
    let loaded: Workspace = reopened.store().load(DocKey::Workspace).unwrap().unwrap();
    let titles: Vec<String> = loaded
        .root_pages()
        .unwrap()
        .into_iter()
        .map(|id| loaded.page(id).unwrap().title)
        .collect();
    assert_eq!(
        titles,
        vec!["Saved"],
        "saved work is back; the unsaved page never reached disk"
    );
}

type Fallible<T> = Result<T, Box<dyn std::error::Error>>;

/// Saves `count` edits to a page, one update row each, and returns the folder path and key.
fn page_with_updates(dir: &TempDir, count: usize) -> Fallible<(PathBuf, DocKey)> {
    let path = dir.0.join("w.leaf");
    let mut folder = Folder::create(&path)?;
    let key = DocKey::Page(PageId::new());
    let mut page = PageDoc::new();
    folder.store_mut().save(key, &page, 0)?;
    for i in 0..count {
        append(&mut page, format!("edit {i}")).map_err(|e| e.to_string())?;
        folder.store_mut().save(key, &page, 1)?;
    }
    Ok((path, key))
}

fn damage(path: &Path, sql: &str) -> Fallible<()> {
    Connection::open(path.join("workspace.sqlite"))?.execute_batch(sql)?;
    Ok(())
}

#[test]
fn a_damaged_trailing_update_is_skipped_and_kept_on_disk() {
    let dir = TempDir::new();
    let (path, key) = page_with_updates(&dir, 3).unwrap();
    // A torn last write: half its bytes, garbled.
    damage(
        &path,
        "UPDATE doc_updates SET data = CAST(substr(data, 1, length(data) / 2) || x'FF' AS BLOB)
         WHERE seq = (SELECT MAX(seq) FROM doc_updates)",
    )
    .unwrap();

    let folder = Folder::open(&path).unwrap();
    let recovered = folder.store().recover::<PageDoc>(key).unwrap().unwrap();
    assert_eq!(texts(&recovered.doc), vec!["edit 0", "edit 1"]);
    assert_eq!(recovered.skipped, vec![3], "the seq of the damaged row");
    let loaded: PageDoc = folder.store().load(key).unwrap().unwrap();
    assert_eq!(
        texts(&loaded),
        vec!["edit 0", "edit 1"],
        "load warns and carries on"
    );

    let rows: i64 = Connection::open(path.join("workspace.sqlite"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM doc_updates", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 3, "the damaged row is left on disk, not deleted");
}

#[test]
fn saving_after_a_skipped_update_still_loads() {
    let dir = TempDir::new();
    let (path, key) = page_with_updates(&dir, 2).unwrap();
    damage(
        &path,
        "UPDATE doc_updates SET data = x'00' WHERE seq = (SELECT MAX(seq) FROM doc_updates)",
    )
    .unwrap();
    let mut folder = Folder::open(&path).unwrap();
    let mut page: PageDoc = folder.store().load(key).unwrap().unwrap();
    append(&mut page, "after".into()).unwrap();
    folder.store_mut().save(key, &page, 5).unwrap();
    drop(folder);

    // The bad row is no longer last, and the page still opens with everything else.
    let folder = Folder::open(&path).unwrap();
    let recovered = folder.store().recover::<PageDoc>(key).unwrap().unwrap();
    assert_eq!(texts(&recovered.doc), vec!["edit 0", "after"]);
    assert_eq!(recovered.skipped, vec![2]);
}

#[test]
fn a_damaged_snapshot_is_still_an_error() {
    let dir = TempDir::new();
    let (path, key) = page_with_updates(&dir, 1).unwrap();
    damage(&path, "UPDATE docs SET snapshot = x'0102'").unwrap();
    let folder = Folder::open(&path).unwrap();
    assert!(matches!(
        folder.store().load::<PageDoc>(key),
        Err(StoreError::Model(_))
    ));
}
