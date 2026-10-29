//! The background indexer end to end, on a real thread (E11-S04).

mod common;

use std::fs;
use std::time::Duration;

use common::{Saved, TempDir, new_page, ok, paragraph, within};
use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace};
use leaf_search::{SEARCH_DIR, SearchWorker, Searcher};
use rusqlite::Connection;

/// The product spec's budget: an edit is searchable within a second.
const BUDGET: Duration = Duration::from_secs(1);

/// Generous wait for things that have no budget of their own (a rebuild on a slow CI machine).
const PATIENCE: Duration = Duration::from_secs(10);

/// The pages a fresh read-only connection finds for `query`; empty while there is no index.
fn found(dir: &TempDir, query: &str) -> Vec<PageId> {
    Searcher::open(&dir.0)
        .and_then(|searcher| searcher.search(query))
        .map(|hits| hits.iter().map(|hit| hit.page).collect())
        .unwrap_or_default()
}

fn one_page(saved: &Saved, title: &str, body: &str) -> (Workspace, PageId) {
    let mut ws = Workspace::new();
    let page = new_page(&mut ws, None, title);
    let mut doc = PageDoc::new();
    paragraph(&mut doc, body);
    saved.save_page(page, &doc, 1);
    saved.save_workspace(&ws, 1);
    (ws, page)
}

#[test]
fn a_new_index_is_built_in_the_background_without_a_poke() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips and roses");
    let _worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());
}

#[test]
fn an_edit_is_searchable_within_a_second_of_the_poke() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips");
    let worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());

    let mut doc = PageDoc::new();
    paragraph(&mut doc, "daffodils now");
    saved.save_page(page, &doc, 2);
    worker.poke();
    let took = within(BUDGET, || found(&dir, "daffodils") == [page]);
    assert!(took.is_some(), "not searchable within {BUDGET:?}");
}

#[test]
fn a_deleted_index_folder_is_rebuilt_on_the_next_start() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips");
    let worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());
    drop(worker); // the app closes

    ok(fs::remove_dir_all(dir.0.join(SEARCH_DIR)));
    assert!(found(&dir, "tulips").is_empty(), "the old index is really gone");
    let _worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());
}

#[test]
fn a_damaged_index_is_thrown_away_and_rebuilt() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips");
    let worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());

    // Something outside Leaf damages the file while it is open.
    let raw = ok(Connection::open(
        dir.0.join(SEARCH_DIR).join(leaf_search::INDEX_FILE),
    ));
    ok(raw.execute_batch("DROP TABLE blocks_fts; DROP TABLE blocks;"));
    drop(raw);
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "daffodils");
    saved.save_page(page, &doc, 2);
    worker.poke();
    assert!(within(PATIENCE, || found(&dir, "daffodils") == [page]).is_some());
}

#[test]
fn a_failing_source_never_stops_the_worker() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips");
    saved.set_failing(true);
    let worker = SearchWorker::start(dir.0.clone(), saved.clone());
    worker.poke();
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        found(&dir, "tulips").is_empty(),
        "nothing indexed while the source fails"
    );

    saved.set_failing(false);
    worker.poke();
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());
}

#[test]
fn pokes_sent_before_closing_are_still_applied() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, page) = one_page(&saved, "Garden", "tulips");
    let worker = SearchWorker::start(dir.0.clone(), saved.clone());
    assert!(within(PATIENCE, || found(&dir, "tulips") == [page]).is_some());

    let mut doc = PageDoc::new();
    paragraph(&mut doc, "last words before quitting");
    saved.save_page(page, &doc, 3);
    worker.poke();
    drop(worker); // waits for the thread
    assert_eq!(found(&dir, "quitting"), [page]);
}
