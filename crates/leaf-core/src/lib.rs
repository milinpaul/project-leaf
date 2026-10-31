//! Shared building blocks for every other Leaf crate: IDs, error types and small primitives.
//!
//! Everything in Leaf depends on this crate, so it stays tiny and dependency-light. Keeping IDs
//! and errors here, rather than in the model, lets storage and search name a page without
//! pulling in the whole document model.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! It depends on no other Leaf crate.

// Core crates must not panic on bad input, so indexing and arithmetic are checked here (E08-S15).
// Unit tests are exempt, like the workspace exempts them from unwrap/expect.
#![cfg_attr(not(test), warn(clippy::indexing_slicing, clippy::arithmetic_side_effects))]

mod asset;
mod error;
mod ids;

pub use asset::AssetId;
pub use error::CoreError;
pub use ids::{BlockId, ColumnId, DatabaseId, OptionId, PageId};
