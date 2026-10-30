//! A scratch folder for one bench run, so benches that save never touch a real workspace.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh path in the system's temp folder, removed (with anything saved there) when dropped.
/// Saves a `tempfile` dependency.
#[derive(Debug)]
pub struct Scratch(PathBuf);

impl Scratch {
    /// A path that doesn't exist yet, unique to this process and call.
    #[must_use]
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "leaf-bench-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }

    /// Where the scratch folder is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
