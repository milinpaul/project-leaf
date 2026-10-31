//! Large, repeatable test workspaces: a page with thousands of blocks, a workspace with thousands of
//! pages, and both together, for performance tests and benchmarks (E08-S01).
//!
//! Everything is built through the real model commands, so a fixture is a normal document: it saves,
//! loads, indexes and renders like one a person made. A fixture is built from a seed: the same seed
//! always gives the same outline (page tree, titles, icons, block kinds, nesting, text and
//! formatting), so a slow run can be repeated exactly. Page and block IDs are minted by the model and
//! differ between runs; nothing should depend on them.
//!
//! **Boundaries:** test support only; apps never depend on it. Native only (it can save to a
//! workspace folder through `leaf-store`). No GPUI and no `leaf-ui`.

mod disk;
mod links;
mod page;
mod rng;
mod words;
mod workspace;

pub use disk::save_to_folder;
pub use links::add_links;
pub use page::{big_page, fill_page};
pub use rng::Rng;
pub use workspace::{Fixture, big_workspace, page_tree};

/// The size the product spec's performance budgets are measured at: a page this long must scroll
/// and type smoothly (`PRODUCT_SPEC.md`, "Performance").
pub const BIG_PAGE_BLOCKS: usize = 10_000;

/// A workspace this large must open, switch pages and search within budget.
pub const BIG_WORKSPACE_PAGES: usize = 5_000;

/// Blocks per page in the full-size workspace: with [`BIG_WORKSPACE_PAGES`] pages, about 100,000
/// blocks in all, the search budget's fixture (E11-S07).
pub const BLOCKS_PER_PAGE: usize = 20;

/// Links each page of the full-size workspace gets for the backlinks budget: 25,000 in all (E10-S06).
pub const LINKS_PER_PAGE: usize = 5;

/// Why a fixture couldn't be built or saved. Building only fails if the model rejects a command,
/// which would be a bug in the generator or the model.
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    /// The model refused a command.
    #[error("model error: {0}")]
    Model(#[from] leaf_model::ModelError),
    /// The model answered a command in a way it never should (a model bug).
    #[error("unexpected model answer: {0}")]
    Unexpected(String),
    /// Saving to a workspace folder failed.
    #[error("store error: {0}")]
    Store(#[from] leaf_store::StoreError),
}
