//! Benchmarks for the performance budgets in `PRODUCT_SPEC.md` (E08-S02), and the setup they share.
//!
//! The benches themselves live in `benches/budgets.rs` (divan) and `benches/search.rs` (search on a
//! 5,000-page workspace, E11-S07, with its own harness because its budget is a 95th percentile).
//! They run with `just bench`; CI runs them on every push to `main` and keeps the results as the
//! `bench-results` artifact. This library holds
//! only their setup: a big page saved to a scratch workspace folder, and an editor with the caret
//! in the middle of a big page. It is tested like any other code, so a bench never measures a
//! setup that quietly went wrong (an empty page, a caret that never landed).
//!
//! Every bench drives the same public calls the app does (`Store::load`, `PageEditor`,
//! `Store::save`), so the numbers are the app's, minus drawing the window.
//!
//! **Boundaries:** test support only; apps never depend on it. Native only (it saves to a folder
//! through `leaf-store`). No GPUI and no `leaf-ui`.

mod scratch;
mod setup;

pub use scratch::Scratch;
pub use setup::{SavedPage, ViewedPage, caret_mid_page, typing_editor};

/// The seed every bench builds its fixture from, so runs measure the same page.
pub const SEED: u64 = 2026;

/// Blocks on the page the "open a page" budget is measured at: < 100 ms (`PRODUCT_SPEC.md`).
pub const OPEN_PAGE_BLOCKS: usize = 5_000;

/// When the benches say a change was saved (2026-01-01, Unix milliseconds): fixed, so a run doesn't
/// depend on the clock.
pub const SAVED_AT: i64 = 1_767_225_600_000;

/// Why a bench's setup failed. Each is a bug in the fixtures, the model or the store, not a slow run.
#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    /// The fixture couldn't be built or saved.
    #[error("fixture error: {0}")]
    Fixture(#[from] leaf_fixtures::FixtureError),
    /// The store refused to save or load.
    #[error("store error: {0}")]
    Store(#[from] leaf_store::StoreError),
    /// The model refused a command.
    #[error("model error: {0}")]
    Model(#[from] leaf_model::ModelError),
    /// The editor refused a call.
    #[error("editor error: {0}")]
    Editor(#[from] leaf_editor::EditorError),
    /// The setup didn't produce what a bench needs (a saved page, a block to type in).
    #[error("bench setup went wrong: {0}")]
    Setup(&'static str),
}
