//! Full-text search across a Leaf workspace. Phase 1; empty until then.
//!
//! The search index is disposable: it can always be rebuilt from the documents, so it never
//! holds data that exists nowhere else. Keeping it separate means the heavy indexing library
//! stays out of the model, the editor and the WASM build.
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.
