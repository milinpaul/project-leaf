//! Keeping the index in step with saved pages (E11-S04): the reconcile logic, without threads.

mod common;

use common::{Saved, TempDir, new_page, ok, paragraph};
use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace, WorkspaceCommand};
use leaf_search::{Finished, IndexState, SearchError, SearchIndex, Syncer, UNTITLED};

fn syncer(dir: &TempDir, saved: &Saved) -> Syncer<Saved> {
    Syncer::new(ok(SearchIndex::open(&dir.0)).0, saved.clone())
}

fn found(syncer: &Syncer<Saved>, query: &str) -> Vec<PageId> {
    ok(syncer.index().search(query))
        .iter()
        .map(|hit| hit.page)
        .collect()
}

const NEVER: &dyn Fn() -> bool = &|| false;

/// A workspace with a page "Recipes" (blocks saved) and a child page "Soups" (blocks saved).
fn sample(saved: &Saved) -> (Workspace, PageId, PageId) {
    let mut ws = Workspace::new();
    let recipes = new_page(&mut ws, None, "Recipes");
    let soups = new_page(&mut ws, Some(recipes), "Soups");
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "bread and butter");
    saved.save_page(recipes, &doc, 10);
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "tomato soup");
    saved.save_page(soups, &doc, 11);
    saved.save_workspace(&ws, 12);
    (ws, recipes, soups)
}

#[test]
fn a_rebuild_indexes_every_page_and_finishes() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (mut ws, recipes, soups) = sample(&saved);
    let empty = new_page(&mut ws, None, "");
    saved.save_workspace(&ws, 13);
    let mut sync = syncer(&dir, &saved);

    assert_eq!(sync.rebuild(NEVER).unwrap(), Finished::Done);
    assert_eq!(found(&sync, "butter"), [recipes]);
    assert_eq!(found(&sync, "tomato"), [soups], "nested pages too");
    assert_eq!(
        found(&sync, &UNTITLED.to_lowercase()),
        [empty],
        "a page with no saved blocks, by title"
    );
    drop(sync);
    assert_eq!(SearchIndex::open(&dir.0).unwrap().1, IndexState::Current);
}

#[test]
fn an_edit_re_reads_only_that_page() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_ws, recipes, _soups) = sample(&saved);
    let mut sync = syncer(&dir, &saved);
    sync.rebuild(NEVER).unwrap();
    let loads = saved.page_loads();

    sync.reconcile(NEVER).unwrap();
    assert_eq!(saved.page_loads(), loads, "nothing changed, nothing read");

    let mut doc = PageDoc::new();
    paragraph(&mut doc, "sourdough starter");
    saved.save_page(recipes, &doc, 20);
    sync.reconcile(NEVER).unwrap();
    assert_eq!(saved.page_loads(), loads + 1);
    assert_eq!(found(&sync, "sourdough"), [recipes]);
    assert!(found(&sync, "butter").is_empty(), "the old text is gone");
}

#[test]
fn renaming_updates_the_title() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (mut ws, recipes, _) = sample(&saved);
    let mut sync = syncer(&dir, &saved);
    sync.rebuild(NEVER).unwrap();

    ws.apply(WorkspaceCommand::RenamePage {
        page: recipes,
        title: "Baking".into(),
    })
    .unwrap();
    saved.save_workspace(&ws, 30);
    sync.reconcile(NEVER).unwrap();
    assert_eq!(found(&sync, "baking"), [recipes]);
    assert!(found(&sync, "recipes").is_empty(), "the old title is gone");
}

#[test]
fn trashing_removes_a_page_and_those_under_it_and_restoring_brings_them_back() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (mut ws, recipes, soups) = sample(&saved);
    let mut sync = syncer(&dir, &saved);
    sync.rebuild(NEVER).unwrap();

    ws.apply(WorkspaceCommand::TrashPage {
        page: recipes,
        at: 40,
    })
    .unwrap();
    saved.save_workspace(&ws, 40);
    sync.reconcile(NEVER).unwrap();
    assert!(
        found(&sync, "butter").is_empty(),
        "trashed pages are not searched"
    );
    assert!(
        found(&sync, "tomato").is_empty(),
        "the child is hidden with its parent"
    );

    ws.apply(WorkspaceCommand::RestorePage { page: recipes }).unwrap();
    saved.save_workspace(&ws, 41);
    sync.reconcile(NEVER).unwrap();
    assert_eq!(found(&sync, "butter"), [recipes]);
    assert_eq!(found(&sync, "tomato"), [soups]);
}

#[test]
fn deleting_for_good_removes_the_page() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (mut ws, recipes, soups) = sample(&saved);
    let mut sync = syncer(&dir, &saved);
    sync.rebuild(NEVER).unwrap();

    ws.apply(WorkspaceCommand::TrashPage { page: soups, at: 50 })
        .unwrap();
    ws.apply(WorkspaceCommand::DeletePagePermanently { page: soups })
        .unwrap();
    saved.save_workspace(&ws, 50);
    saved.forget_page(soups);
    sync.reconcile(NEVER).unwrap();
    assert!(found(&sync, "tomato").is_empty(), "deleted for good");
    assert!(!sync.index().indexed_pages().unwrap().contains_key(&soups));
    assert_eq!(found(&sync, "butter"), [recipes]);
}

#[test]
fn saves_the_index_missed_are_caught_up_on_the_next_start() {
    // As after a crash: pages were saved but the app died before the index heard of it.
    let (dir, saved) = (TempDir::new(), Saved::default());
    let (_, recipes, _) = sample(&saved);
    syncer(&dir, &saved).rebuild(NEVER).unwrap();
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "saved before the crash");
    saved.save_page(recipes, &doc, 60);

    let (index, state) = SearchIndex::open(&dir.0).unwrap();
    assert_eq!(state, IndexState::Current, "no rebuild needed for this");
    let mut sync = Syncer::new(index, saved.clone());
    sync.reconcile(NEVER).unwrap();
    assert_eq!(found(&sync, "crash"), [recipes]);
}

#[test]
fn a_stopped_rebuild_is_left_incomplete() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    sample(&saved);
    let mut sync = syncer(&dir, &saved);
    assert_eq!(sync.rebuild(&|| true).unwrap(), Finished::Stopped);
    drop(sync);
    assert_eq!(SearchIndex::open(&dir.0).unwrap().1, IndexState::NeedsRebuild);
}

#[test]
fn a_source_that_cannot_be_read_is_an_error() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    sample(&saved);
    let mut sync = syncer(&dir, &saved);
    saved.set_failing(true);
    let err = sync.reconcile(NEVER).unwrap_err();
    assert!(matches!(err, SearchError::Source(_)));
    assert!(
        err.to_string()
            .starts_with("search could not read the saved pages")
    );
}

#[test]
fn rows_of_a_database_are_found_though_the_sidebar_leaves_them_out() {
    let (dir, saved) = (TempDir::new(), Saved::default());
    let mut ws = Workspace::new();
    let Ok(leaf_model::Outcome::DatabasePageCreated { page: list, .. }) =
        ws.apply(WorkspaceCommand::CreateDatabasePage {
            parent: None,
            index: 0,
            title: "Reading list".into(),
            created_at: 1,
        })
    else {
        panic!("expected a database page");
    };
    let Ok(leaf_model::Outcome::PageCreated(row)) = ws.apply_unrecorded(WorkspaceCommand::CreateRowPage {
        database_page: list,
        created_at: 2,
    }) else {
        panic!("expected a row page");
    };
    ws.apply(WorkspaceCommand::RenamePage {
        page: row,
        title: "Dune".into(),
    })
    .unwrap();
    let mut doc = PageDoc::new();
    paragraph(&mut doc, "sand worms");
    saved.save_page(row, &doc, 3);
    saved.save_workspace(&ws, 4);
    let mut sync = syncer(&dir, &saved);
    assert_eq!(sync.rebuild(NEVER).unwrap(), Finished::Done);
    assert_eq!(found(&sync, "worms"), [row], "by its text");
    assert_eq!(found(&sync, "dune"), [row], "by its title");
}
