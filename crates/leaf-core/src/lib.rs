//! Shared building blocks for every other Leaf crate: IDs, error types and small primitives.
//!
//! Everything in Leaf depends on this crate, so it stays tiny and dependency-light. Keeping IDs
//! and errors here, rather than in the model, lets storage and search name a page without
//! pulling in the whole document model.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! It depends on no other Leaf crate.
