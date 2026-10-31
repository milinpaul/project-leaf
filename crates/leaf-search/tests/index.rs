//! Opening, recreating and writing the search index file (E11-S01).

mod common;

use std::fs;
use std::path::Path;

use common::TempDir;

use leaf_core::{BlockId, PageId};
use leaf_search::{
    BlockText, INDEX_VERSION, IndexState, IndexedPage, PageText, PageVersion, SEARCH_DIR, SearchError,
    SearchIndex, Searcher,
};
use rusqlite::Connection;

fn open(dir: &TempDir) -> (SearchIndex, IndexState) {
    match SearchIndex::open(&dir.0) {
        Ok(opened) => opened,
        Err(err) => panic!("open failed: {err}"),
    }
}

/// Indexes `text` as if from a page's first save.
fn put(index: &mut SearchIndex, text: &PageText) -> Result<(), SearchError> {
    index.index_page(text, PageVersion::default())
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
        links: Vec::new(),
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
    put(&mut index, &page("Café plans", &["Résumé for the PROJECT"])).unwrap();

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
    put(&mut index, &page("Notes", &["kept across opens"])).unwrap();
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
    put(&mut index, &page("Half", &["done"])).unwrap();
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
    put(&mut index, &page("Fresh", &["works again"])).unwrap();
    assert_eq!(block_hits(&path, "works"), 1);
}

#[test]
fn an_index_of_another_version_is_replaced() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    put(&mut index, &page("Old", &["from an older index"])).unwrap();
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
    put(&mut index, &notes).unwrap();
    notes.title = "Final".to_owned();
    notes.blocks.truncate(1);
    notes.blocks[0].text = "second words".to_owned();
    put(&mut index, &notes).unwrap();

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
    put(&mut index, &gone).unwrap();
    put(&mut index, &page("Stays", &["shared word"])).unwrap();

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
    put(&mut index, &page("One", &["alpha"])).unwrap();
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
    put(&mut index, &notes).unwrap();
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
    let err = put(&mut index, &notes).unwrap_err();
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

#[test]
fn indexed_pages_report_title_and_version() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    let notes = page("Notes", &["body"]);
    let version = PageVersion {
        saved_at: 1_700,
        seq: 42,
    };
    index.index_page(&notes, version).unwrap();

    let pages = index.indexed_pages().unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(
        pages[&notes.page],
        IndexedPage {
            title: "Notes".to_owned(),
            version
        }
    );

    raw(&dir.index_path())
        .execute(
            "INSERT INTO pages (page_id, title, saved_at, saved_seq) VALUES ('junk', 'x', 0, 0)",
            [],
        )
        .unwrap();
    assert_eq!(
        index.indexed_pages().unwrap().len(),
        1,
        "unparsable IDs are left out"
    );
}

#[test]
fn recreating_resets_the_tables_and_starts_empty() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    put(&mut index, &page("Gone", &["soon"])).unwrap();
    index.mark_complete().unwrap();
    raw(&dir.index_path())
        .execute_batch("DROP TABLE blocks_fts;")
        .unwrap();
    assert!(index.search("soon").is_err(), "the damage is real");

    let searcher = Searcher::open(&dir.0).unwrap(); // as the search bar keeps one open
    index.recreate().unwrap();
    assert!(index.indexed_pages().unwrap().is_empty());
    put(&mut index, &page("Back", &["working"])).unwrap();
    assert_eq!(index.search("working").unwrap().len(), 1);
    assert_eq!(
        searcher.search("working").unwrap().len(),
        1,
        "an open reader keeps working"
    );
    drop(index);
    assert_eq!(
        open(&dir).1,
        IndexState::NeedsRebuild,
        "a recreated index is due a rebuild"
    );
}

#[test]
fn recreating_replaces_a_file_that_is_no_longer_a_database() {
    let dir = TempDir::new();
    let (mut index, _) = open(&dir);
    put(&mut index, &page("Gone", &["soon"])).unwrap();
    fs::write(dir.index_path(), vec![b'x'; 8192]).unwrap();

    index.recreate().unwrap();
    put(&mut index, &page("Back", &["working"])).unwrap();
    assert_eq!(index.search("working").unwrap().len(), 1);
    assert!(
        index.search("soon").unwrap().is_empty(),
        "the old rows went with the old file"
    );
}
