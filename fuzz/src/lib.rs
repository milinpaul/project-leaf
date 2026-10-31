//! Shared pieces of Leaf's fuzz targets (E08-S11): how one fuzz input is cut into a document's saved parts,
//! a scratch folder per run, and "walks" that read every part of a loaded document the way the app would.
//!
//! A target fails only by panicking, hanging or running out of memory; libFuzzer catches all three. Errors
//! are the right answer to bad bytes, so the walks ignore them and keep reading.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Database, PageDoc, Workspace};

/// Puts Rust's normal panic hook back in place of libFuzzer's, which aborts on every panic, even one Leaf
/// catches on purpose (a panic inside Loro becomes an error, ADR-088). A panic that escapes a target
/// still aborts the run and counts as a finding: libFuzzer catches it at the top and aborts there. Call it
/// first in every target.
pub fn allow_caught_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| drop(std::panic::take_hook()));
}

/// Separates a document's snapshot from its later updates in a `doc_bytes` input, the way the store keeps
/// them in separate rows. Long and odd enough not to turn up inside real Loro bytes by chance.
pub const PART: &[u8] = b"\0LEAF-UPDATE\0";

/// Cuts `bytes` at every [`PART`]: the first piece is the snapshot, the rest are updates.
#[must_use]
pub fn parts(bytes: &[u8]) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    let mut rest = bytes;
    while let Some(at) = rest.windows(PART.len()).position(|window| window == PART) {
        pieces.push(&rest[..at]);
        rest = &rest[at + PART.len()..];
    }
    pieces.push(rest);
    pieces
}

/// An empty folder for this run, inside one temporary folder per fuzzing process. Emptied each time, so
/// nothing piles up on disk over millions of runs.
#[must_use]
pub fn scratch(name: &str) -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    let root = ROOT.get_or_init(|| std::env::temp_dir().join(format!("leaf-fuzz-{}", std::process::id())));
    let folder = root.join(name);
    let _ = std::fs::remove_dir_all(&folder);
    let _ = std::fs::create_dir_all(&folder);
    folder
}

/// Reads every block of `page`: kinds, text, links and table cells, then checks its rules and that it saves
/// and loads again. A page Leaf has loaded must always be savable, or the owner's next edit would be lost.
pub fn walk_page(page: &PageDoc) {
    let mut seen = HashSet::new();
    let mut todo: Vec<BlockId> = page.root_blocks();
    while let Some(block) = todo.pop() {
        if !seen.insert(block) {
            continue;
        }
        let _ = page.parent(block);
        let _ = page.text(block);
        let _ = page.plain_text(block);
        if matches!(page.kind(block), Ok(BlockKind::Table { .. })) {
            let _ = page.table_cells(block);
        }
        if let Ok(children) = page.children(block) {
            todo.extend(children);
        }
    }
    let _ = page.page_links();
    let _ = page.check_invariants();
    let bytes = page
        .to_bytes()
        .unwrap_or_else(|err| panic!("a loaded page doesn't save: {err}"));
    if let Err(err) = PageDoc::from_bytes(&bytes) {
        panic!("a saved page doesn't load again: {err}");
    }
}

/// Reads the whole page tree of `workspace`, its trash and favourites, and returns the pages and databases
/// it names, so a caller can load those too.
pub fn walk_workspace(workspace: &Workspace) -> (Vec<PageId>, Vec<leaf_core::DatabaseId>) {
    let mut seen = HashSet::new();
    let mut databases = Vec::new();
    let mut todo = workspace.root_pages().unwrap_or_default();
    todo.extend(workspace.trash().unwrap_or_default());
    todo.extend(workspace.favourites().unwrap_or_default());
    while let Some(page) = todo.pop() {
        if !seen.insert(page) {
            continue;
        }
        let _ = workspace.parent(page);
        let _ = workspace.is_in_trash(page);
        if let Ok(meta) = workspace.page(page) {
            for database in meta.database.into_iter().chain(meta.row_of) {
                if !databases.contains(&database) {
                    databases.push(database);
                    todo.extend(workspace.row_pages(database).unwrap_or_default());
                }
            }
        }
        if let Ok(children) = workspace.children(page) {
            todo.extend(children);
        }
    }
    let _ = workspace.child_slots(None);
    let bytes = workspace
        .to_bytes()
        .unwrap_or_else(|err| panic!("a loaded workspace doesn't save: {err}"));
    if let Err(err) = Workspace::from_bytes(&bytes) {
        panic!("a saved workspace doesn't load again: {err}");
    }
    (seen.into_iter().collect(), databases)
}

/// Reads every column, row and cell of `database`, its sort and filters, and checks its rules and that it
/// saves and loads again.
pub fn walk_database(database: &Database) {
    let _ = database.columns();
    for column in database.column_ids() {
        let _ = database.column(column);
    }
    for row in database.rows() {
        let _ = database.row_values(row);
    }
    let _ = database.sort();
    let _ = database.filters();
    let _ = database.check_invariants();
    let bytes = database
        .to_bytes()
        .unwrap_or_else(|err| panic!("a loaded database doesn't save: {err}"));
    if let Err(err) = Database::from_bytes(&bytes) {
        panic!("a saved database doesn't load again: {err}");
    }
}
