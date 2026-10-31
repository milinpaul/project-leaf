//! Finding links (symlinks, and on Windows junctions) inside a workspace folder before opening it.
//!
//! Leaf writes its database, lock, view state, images and search index by fixed names inside the
//! workspace. If one of those names were a link, the write would land wherever the link points:
//! a crafted workspace could make Leaf create or overwrite files anywhere the owner can write.
//! The owner chose (ADR-074) that such a workspace is refused rather than repaired, so nothing in
//! it is changed. The workspace folder itself may be a link: the owner picked it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{ASSETS, SEARCH};

/// The first link found directly in `root`, or directly in its `assets/` and `search/` folders
/// (the only places Leaf writes). Folders that don't exist yet are fine: opening creates them.
pub(super) fn first_link(root: &Path) -> io::Result<Option<PathBuf>> {
    for folder in [root.to_owned(), root.join(ASSETS), root.join(SEARCH)] {
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                return Ok(Some(entry.path()));
            }
        }
    }
    Ok(None)
}
