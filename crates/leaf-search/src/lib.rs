//! Full-text search across a Leaf workspace (SQLite FTS5, ADR 0019). Empty until epic E11 lands.
//!
//! The search index is disposable: it can always be rebuilt from the documents, so it never
//! holds data that exists nowhere else. Keeping it separate means the heavy indexing library
//! stays out of the model, the editor and the WASM build.
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.

#[cfg(test)]
mod tests {
    /// Placeholder so the coverage gate has something to measure while the crate is empty.
    /// Delete it once the crate has real code and tests.
    #[test]
    fn crate_builds() {}
}
