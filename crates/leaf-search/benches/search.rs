//! Search against its budget on the full-size workspace (E11-S07): 5,000 pages, 100,000 blocks.
//!
//! Measures a full rebuild of the index, typed queries through the search bar's read-only
//! connection (p95 must stay under 50 ms) and re-indexing one edited page the way the background
//! worker does after a save (p95 under 20 ms). Prints the numbers and fails when a budget is missed.
//! Run with `just bench-search` (or `cargo bench -p leaf-search`); timings only mean something in
//! the optimised build `cargo bench` uses.

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use leaf_core::PageId;
use leaf_fixtures::{BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE, big_workspace};
use leaf_model::{PageDoc, Workspace};
use leaf_search::{PageSource, PageVersion, SavedVersions, SearchIndex, Searcher, SourceError, Syncer};

/// The fixture's seed: a fixed one, so every run measures the same workspace.
const SEED: u64 = 42;
/// Budget for one query, at the 95th percentile.
const QUERY_BUDGET: Duration = Duration::from_millis(50);
/// Budget for re-indexing one edited page, at the 95th percentile.
const REINDEX_BUDGET: Duration = Duration::from_millis(20);
/// Times each query is run.
const QUERY_RUNS: usize = 20;
/// Edits re-indexed.
const REINDEX_RUNS: usize = 100;

/// What people type: single words, two words, a prefix still being typed, accented and non-Latin
/// words, very common words that match nearly every page, and a word that matches nothing.
const QUERIES: &[&str] = &[
    "garden",
    "meeting notes",
    "tom",
    "release checklist",
    "café",
    "zurich",
    "東京",
    "привет",
    "bug fix",
    "the",
    "a",
    "project plan review",
    "nothingmatchesthis",
];

/// Saved documents held in memory, as the store would hand them to the indexer: bytes and versions.
#[derive(Clone, Default)]
struct Saved(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    workspace: Vec<u8>,
    pages: HashMap<PageId, (Vec<u8>, PageVersion)>,
    seq: i64,
}

impl Saved {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save_page(&self, page: PageId, bytes: Vec<u8>) {
        let mut state = self.state();
        state.seq += 1;
        let version = PageVersion {
            saved_at: state.seq,
            seq: state.seq,
        };
        state.pages.insert(page, (bytes, version));
    }
}

impl PageSource for Saved {
    fn versions(&mut self) -> Result<SavedVersions, SourceError> {
        let state = self.state();
        Ok(SavedVersions {
            workspace: Some(PageVersion { saved_at: 0, seq: 0 }),
            pages: state
                .pages
                .iter()
                .map(|(page, (_, version))| (*page, *version))
                .collect(),
        })
    }

    fn workspace(&mut self) -> Result<Workspace, SourceError> {
        Ok(Workspace::from_bytes(&self.state().workspace)?)
    }

    fn page(&mut self, page: PageId) -> Result<Option<PageDoc>, SourceError> {
        match self.state().pages.get(&page) {
            Some((bytes, _)) => Ok(Some(PageDoc::from_bytes(bytes)?)),
            None => Ok(None),
        }
    }
}

/// A folder for the index, removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The time below which `share` of `times` fall (`times` sorted).
fn percentile(times: &[Duration], share: f64) -> Duration {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a few thousand samples"
    )]
    let at = ((times.len() as f64 * share).ceil() as usize).clamp(1, times.len());
    times[at - 1]
}

fn ms(time: Duration) -> String {
    format!("{:.2} ms", time.as_secs_f64() * 1000.0)
}

/// Prints p50, p95 and the worst of `times` and says whether p95 is within `budget`.
fn report(what: &str, times: &mut [Duration], budget: Duration) -> bool {
    times.sort_unstable();
    let p95 = percentile(times, 0.95);
    let ok = p95 < budget;
    println!(
        "{what}: p50 {}, p95 {}, worst {} over {} runs; budget p95 < {} {}",
        ms(percentile(times, 0.5)),
        ms(p95),
        ms(times[times.len() - 1]),
        times.len(),
        ms(budget),
        if ok { "OK" } else { "MISSED" }
    );
    ok
}

fn run() -> Result<bool, Box<dyn Error + Send + Sync>> {
    let started = Instant::now();
    let fixture = big_workspace(SEED, BIG_WORKSPACE_PAGES, BLOCKS_PER_PAGE)?;
    println!(
        "fixture: {} pages, {} blocks (seed {SEED}), built in {:.1} s",
        fixture.pages.len(),
        fixture.block_count(),
        started.elapsed().as_secs_f64()
    );
    let saved = Saved::default();
    saved.state().workspace = fixture.workspace.to_bytes()?;
    for (page, doc) in &fixture.pages {
        saved.save_page(*page, doc.to_bytes()?);
    }

    let dir = TempDir(std::env::temp_dir().join(format!("leaf-search-bench-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    std::fs::create_dir_all(&dir.0)?;
    let (index, _) = SearchIndex::open(&dir.0)?;
    let mut syncer = Syncer::new(index, saved.clone());

    let started = Instant::now();
    syncer.rebuild(&|| false)?;
    println!("full rebuild: {:.2} s", started.elapsed().as_secs_f64());

    let searcher = Searcher::open(&dir.0)?;
    let mut times = Vec::with_capacity(QUERIES.len() * QUERY_RUNS);
    for query in QUERIES {
        let mut slowest = Duration::ZERO;
        let mut found = 0;
        for _ in 0..QUERY_RUNS {
            let started = Instant::now();
            found = searcher.search(query)?.len();
            let took = started.elapsed();
            slowest = slowest.max(took);
            times.push(took);
        }
        println!("  {query:>22}: {found:>3} pages, slowest {}", ms(slowest));
    }
    let queries_ok = report("query", &mut times, QUERY_BUDGET);

    // Two different pages' blocks, saved in turn under one page, so every round really changes it.
    let (edited, first) = &fixture.pages[0];
    let (_, second) = &fixture.pages[1];
    let versions = [first.to_bytes()?, second.to_bytes()?];
    let mut times = Vec::with_capacity(REINDEX_RUNS);
    for round in 0..REINDEX_RUNS {
        saved.save_page(*edited, versions[(round + 1) % 2].clone());
        let started = Instant::now();
        syncer.reconcile(&|| false)?;
        times.push(started.elapsed());
    }
    let reindex_ok = report("re-index one edited page", &mut times, REINDEX_BUDGET);
    Ok(queries_ok && reindex_ok)
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("search missed its budget");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("search benchmark failed: {err}");
            ExitCode::FAILURE
        }
    }
}
