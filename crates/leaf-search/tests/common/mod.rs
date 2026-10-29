//! Helpers shared by the integration tests. Each test binary uses only some of them.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use leaf_core::{BlockId, PageId};
use leaf_model::{BlockKind, Outcome, PageCommand, PageDoc, PageOutcome, Workspace, WorkspaceCommand};
use leaf_search::{INDEX_FILE, PageSource, PageVersion, SEARCH_DIR, SavedVersions, SourceError};

/// A fresh, empty directory for one test, removed when dropped. Saves a `tempfile` dependency.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "leaf-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            panic!("can't create {}: {err}", dir.display());
        }
        Self(dir)
    }

    pub fn index_path(&self) -> PathBuf {
        self.0.join(SEARCH_DIR).join(INDEX_FILE)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Unwraps a helper's result. Helpers aren't `#[test]` functions, so clippy's test allowance
/// for `unwrap` doesn't reach them.
pub fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|err| panic!("{err}"))
}

pub fn new_page(ws: &mut Workspace, parent: Option<PageId>, title: &str) -> PageId {
    let outcome = ok(ws.apply(WorkspaceCommand::CreatePage {
        parent,
        index: 0,
        title: title.to_owned(),
        created_at: 1,
    }));
    let Outcome::PageCreated(id) = outcome else {
        panic!("expected a new page, got {outcome:?}")
    };
    id
}

pub fn add(doc: &mut PageDoc, kind: BlockKind, text: &str) -> BlockId {
    let index = doc.root_blocks().len();
    let outcome = ok(doc.apply(PageCommand::InsertBlock {
        parent: None,
        index,
        kind,
        text: text.to_owned(),
    }));
    let PageOutcome::BlockCreated(id) = outcome else {
        panic!("expected a new block, got {outcome:?}")
    };
    id
}

pub fn paragraph(doc: &mut PageDoc, text: &str) -> BlockId {
    add(doc, BlockKind::Paragraph, text)
}

#[derive(Default)]
struct State {
    workspace: Option<(Vec<u8>, PageVersion)>,
    pages: HashMap<PageId, (Vec<u8>, PageVersion)>,
    seq: i64,
    failing: bool,
    page_loads: usize,
}

/// An in-memory stand-in for the store: "saving" keeps a copy of a document's bytes and gives it
/// a new version, like `leaf_store::Store::saved_versions` would. Clones share the same state, so
/// a test can keep saving while a worker thread reads.
#[derive(Clone, Default)]
pub struct Saved(Arc<Mutex<State>>);

impl Saved {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn next(state: &mut State, saved_at: i64) -> PageVersion {
        state.seq += 1;
        PageVersion {
            saved_at,
            seq: state.seq,
        }
    }

    pub fn save_workspace(&self, ws: &Workspace, saved_at: i64) {
        let mut state = self.state();
        let version = Self::next(&mut state, saved_at);
        state.workspace = Some((ok(ws.to_bytes()), version));
    }

    pub fn save_page(&self, page: PageId, doc: &PageDoc, saved_at: i64) {
        let mut state = self.state();
        let version = Self::next(&mut state, saved_at);
        state.pages.insert(page, (ok(doc.to_bytes()), version));
    }

    pub fn forget_page(&self, page: PageId) {
        self.state().pages.remove(&page);
    }

    /// While set, every read fails, like a database that can't be opened.
    pub fn set_failing(&self, failing: bool) {
        self.state().failing = failing;
    }

    /// How many times a page's blocks were read, to prove unchanged pages are skipped.
    pub fn page_loads(&self) -> usize {
        self.state().page_loads
    }

    fn check(state: &State) -> Result<(), SourceError> {
        if state.failing {
            Err("the test made the source fail".into())
        } else {
            Ok(())
        }
    }
}

impl PageSource for Saved {
    fn versions(&mut self) -> Result<SavedVersions, SourceError> {
        let state = self.state();
        Self::check(&state)?;
        Ok(SavedVersions {
            workspace: state.workspace.as_ref().map(|(_, version)| *version),
            pages: state
                .pages
                .iter()
                .map(|(page, (_, version))| (*page, *version))
                .collect(),
        })
    }

    fn workspace(&mut self) -> Result<Workspace, SourceError> {
        let state = self.state();
        Self::check(&state)?;
        match &state.workspace {
            Some((bytes, _)) => Ok(Workspace::from_bytes(bytes)?),
            None => Ok(Workspace::new()),
        }
    }

    fn page(&mut self, page: PageId) -> Result<Option<PageDoc>, SourceError> {
        let mut state = self.state();
        Self::check(&state)?;
        state.page_loads += 1;
        match state.pages.get(&page) {
            Some((bytes, _)) => Ok(Some(PageDoc::from_bytes(bytes)?)),
            None => Ok(None),
        }
    }
}

/// Polls `condition` until it holds or `limit` passes; returns how long it took, or `None`.
pub fn within(limit: Duration, mut condition: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if condition() {
            return Some(start.elapsed());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}
