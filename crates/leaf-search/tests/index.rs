//! Opening, recreating and writing the search index file (E11-S01).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_core::{BlockId, PageId};
use leaf_search::{
    BlockText, INDEX_FILE, INDEX_VERSION, IndexState, PageText, SEARCH_DIR, SearchError, SearchIndex,
};
use rusqlite::Connection;

/// A fresh, empty directory for one test, removed when dropped. Saves a `tempfile` dependency.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "leaf-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        if let Err(err) = fs::create_dir_all(&dir) {
            panic!("can't create {}: {err}", dir.display());
        }
        Self(dir)
    }

    fn index_path(&self) -> PathBuf {
        self.0.join(SEARCH_DIR).join(INDEX_FILE)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn open(dir: &TempDir) -> (SearchIndex, IndexState) {
    match SearchIndex::open(&dir.0) {
        Ok(opened) => opened,
        Err(err) => panic!("open failed: {err}"),
    }
}

fn page(title: &str, texts: &[&str]) -> PageText {
    PageText {
        page: PageId::new(),
        title: title.to_owned(),
        blocks: texts
            .iter()
            .map(|text| BlockText {
                block: BlockId::new(),
                kind: "paragraph".to_owned(),
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

/// Unwraps a helper's result. Helpers aren't `#[test]` functions, so clippy's test allowance
/// for `unwrap` doesn't reach them.
fn ok<T>(result: rusqlite::Result<T>) -> T {
    result.unwrap_or_else(|err| panic!("sqlite: {err}"))
}

/// A second connection to the index file, as the search side will use.
fn raw(path: &Path) -> Connection {
    ok(Connection::open(path))
}

/// How many rows of an FTS5 table match a query.
fn hits(path: &Path, table: &str, query: &str) -> i64 {
    let sql = format!("SELECT count(*) FROM {table} WHERE {table} MATCH ?1");
    ok(raw(path).query_row(&sql, [query], |row| row.get(0)))
}

/// How many blocks match an FTS5 query.
fn block_hits(path: &Path, query: &str) -> i64 {
    hits(path, "blocks_fts", query)
}

/// How many page titles match an FTS5 query.
fn title_hits(path: &Path, query: &str) -> i64 {
    hits(path, "pages_fts", query)
}

/// FTS5's own check that the full-text tables agree with the text tables.
fn assert_consistent(path: &Path) {
    let conn = raw(path);
    for table in ["blocks_fts", "pages_fts"] {
        ok(conn.execute(
            &format!("INSERT INTO {table}({table}) VALUES ('integrity-check')"),
            [],
        ));
    }
}

#[test]
fn bundled_sqlite_has_fts5_ignoring_case_and_accents_with_prefixes() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    index
        .index_page(&page("Café plans", &["Résumé for the PROJECT"]))
        .unwrap();

    let path = dir.index_path();
    assert_eq!(block_hits(&path, "resume"), 1);
    assert_eq!(block_hits(&path, "project"), 1);
    assert_eq!(block_hits(&path, "proj*"), 1);
    assert_eq!(block_hits(&path, "pr*"), 1);
    assert_eq!(
        block_hits(&path, "plans"),
        0,
        "titles are indexed apart from blocks"
    );
    assert_eq!(title_hits(&path, "cafe"), 1);
    assert_eq!(title_hits(&path, "CAFÉ"), 1);
    assert_eq!(title_hits(&path, "pla*"), 1);
}

#[test]
fn a_new_index_is_created_and_needs_a_rebuild() {
    let dir = TempDir::new();
    let (_index, state) = open(&dir);
    assert_eq!(state, IndexState::NeedsRebuild);
    assert!(dir.index_path().is_file());
}

#[test]
fn an_index_marked_complete_reopens_current_with_its_rows() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    index.index_page(&page("Notes", &["kept across opens"])).unwrap();
    index.mark_complete().unwrap();
    drop(index);

    let (_index, state) = open(&dir);
    assert_eq!(state, IndexState::Current);
    assert_eq!(block_hits(&dir.index_path(), "kept"), 1);
}

#[test]
fn an_interrupted_rebuild_needs_another() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    index.index_page(&page("Half", &["done"])).unwrap();
    drop(index);

    let (_index, state) = open(&dir);
    assert_eq!(state, IndexState::NeedsRebuild);
}

#[test]
fn a_damaged_file_is_replaced_by_an_empty_index() {
    let dir = TempDir::new();
    let path = dir.index_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, vec![b'x'; 4096]).unwrap();
    let mut wal = path.clone().into_os_string();
    wal.push("-wal");
    fs::write(&wal, b"stale").unwrap();

    let (mut index, state) = open(&dir);
    assert_eq!(state, IndexState::NeedsRebuild);
    index.index_page(&page("Fresh", &["works again"])).unwrap();
    assert_eq!(block_hits(&path, "works"), 1);
}

#[test]
fn an_index_of_another_version_is_replaced() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    index.index_page(&page("Old", &["from an older index"])).unwrap();
    index.mark_complete().unwrap();
    drop(index);
    let older = (INDEX_VERSION - 1).to_string();
    raw(&dir.index_path())
        .execute("UPDATE meta SET value = ?1 WHERE key = 'index_version'", [older])
        .unwrap();

    let (_index, state) = open(&dir);
    assert_eq!(state, IndexState::NeedsRebuild);
    assert_eq!(block_hits(&dir.index_path(), "older"), 0);
}

#[test]
fn indexing_a_page_again_replaces_its_rows() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    let mut notes = page("Draft", &["first words", "more first"]);
    index.index_page(&notes).unwrap();
    notes.title = "Final".to_owned();
    notes.blocks.truncate(1);
    notes.blocks[0].text = "second words".to_owned();
    index.index_page(&notes).unwrap();

    let path = dir.index_path();
    assert_eq!(block_hits(&path, "first"), 0);
    assert_eq!(block_hits(&path, "second"), 1);
    assert_eq!(block_hits(&path, "words"), 1);
    assert_eq!(title_hits(&path, "draft"), 0);
    assert_eq!(title_hits(&path, "final"), 1);
    assert_consistent(&path);
}

#[test]
fn removing_a_page_leaves_the_others() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    let gone = page("Gone", &["shared word"]);
    index.index_page(&gone).unwrap();
    index.index_page(&page("Stays", &["shared word"])).unwrap();

    index.remove_page(gone.page).unwrap();
    index.remove_page(PageId::new()).unwrap();

    let path = dir.index_path();
    assert_eq!(block_hits(&path, "shared"), 1);
    assert_eq!(title_hits(&path, "gone"), 0);
    assert_eq!(title_hits(&path, "stays"), 1);
    assert_consistent(&path);
}

#[test]
fn clearing_empties_the_index_and_asks_for_a_rebuild() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    index.index_page(&page("One", &["alpha"])).unwrap();
    index.mark_complete().unwrap();
    index.clear().unwrap();
    drop(index);

    let path = dir.index_path();
    assert_eq!(block_hits(&path, "alpha"), 0);
    assert_eq!(title_hits(&path, "one"), 0);
    assert_consistent(&path);
    let (_index, state) = open(&dir);
    assert_eq!(state, IndexState::NeedsRebuild);
}

#[test]
fn a_failed_index_page_keeps_the_previous_rows() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    let mut notes = page("Before", &["original text"]);
    index.index_page(&notes).unwrap();
    raw(&dir.index_path())
        .execute_batch(
            "CREATE TRIGGER refuse BEFORE INSERT ON blocks WHEN new.text = 'boom'
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .unwrap();

    notes.title = "After".to_owned();
    notes.blocks[0].text = "replacement".to_owned();
    notes.blocks.push(BlockText {
        block: BlockId::new(),
        kind: "code".to_owned(),
        text: "boom".to_owned(),
    });
    let err = index.index_page(&notes).unwrap_err();
    assert!(matches!(err, SearchError::Sqlite(_)));
    assert!(err.to_string().contains("refused"));

    let path = dir.index_path();
    assert_eq!(block_hits(&path, "original"), 1);
    assert_eq!(block_hits(&path, "replacement"), 0);
    assert_eq!(title_hits(&path, "before"), 1);
}

#[test]
fn a_file_where_the_search_folder_should_be_is_an_io_error() {
    let dir = TempDir::new();
    fs::write(dir.0.join(SEARCH_DIR), b"not a folder").unwrap();
    let err = SearchIndex::open(&dir.0).unwrap_err();
    assert!(matches!(err, SearchError::Io(_)));
    assert!(err.to_string().starts_with("search index file system error"));
}
