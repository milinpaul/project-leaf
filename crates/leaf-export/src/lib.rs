//! Markdown import and export for Leaf pages and workspaces. Phase 1; empty until then.
//!
//! Markdown is an exchange format, never the storage format. Keeping conversion in its own crate
//! means the document model never bends to fit markdown, and export stays a pure function that
//! is easy to snapshot-test.
//!
//! **Boundaries:** core crate, wasm-ok. No GPUI, no `leaf-ui`, no file system, threads or SQLite.
//! Depends only on `leaf-core` and `leaf-model`.

#[cfg(test)]
mod tests {
    /// Placeholder so the coverage gate has something to measure while the crate is empty.
    /// Delete it once the crate has real code and tests.
    #[test]
    fn crate_builds() {}
}
