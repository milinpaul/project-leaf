//! Favourite pages: the pages starred in the top bar, listed at the top of the sidebar (ADR-060).
//!
//! Kept in the workspace doc as a map from page ID to position, not as a list. Two devices starring
//! the same page write the same key, so it is listed once; a list would hold it twice. A new
//! favourite takes the highest position plus one, so it goes last; equal positions (two devices
//! starring different pages at the same moment) are ordered by page ID, the same on every device.

use leaf_core::PageId;
use loro::{LoroMap, LoroValue, ValueOrContainer};

use super::Workspace;
use crate::{ModelError, Outcome};

/// Name of the Loro map that holds the favourites.
pub(super) const FAVOURITES: &str = "favourites";

impl Workspace {
    /// Favourite pages in the order they were starred. Pages in the trash (or inside a trashed
    /// page) are left out until restored. So are entries whose page is gone, which a device can be
    /// left with when another deleted the page while it starred it.
    pub fn favourites(&self) -> Result<Vec<PageId>, ModelError> {
        let mut entries = self.favourite_entries();
        entries.sort_unstable_by_key(|&(page, position)| (position, page));
        let mut pages = Vec::with_capacity(entries.len());
        for (page, _) in entries {
            if let Ok(node) = self.node(page)
                && !self.in_trash(node)?
            {
                pages.push(page);
            }
        }
        Ok(pages)
    }

    /// Whether `page` is starred, in the trash or not.
    #[must_use]
    pub fn is_favourite(&self, page: PageId) -> bool {
        position(&self.favourites_map(), page).is_some()
    }

    pub(super) fn add_favourite(&mut self, page: PageId) -> Result<Outcome, ModelError> {
        let node = self.node(page)?;
        if self.in_trash(node)? {
            return Err(ModelError::PageInTrash(page));
        }
        if self.is_favourite(page) {
            return Ok(Outcome::Unchanged);
        }
        let last = self
            .favourite_entries()
            .into_iter()
            .map(|(_, position)| position)
            .max()
            .unwrap_or(0);
        self.favourites_map()
            .insert(&page.to_string(), LoroValue::I64(last.saturating_add(1)))?;
        Ok(Outcome::Changed)
    }

    pub(super) fn remove_favourite(&mut self, page: PageId) -> Result<Outcome, ModelError> {
        let map = self.favourites_map();
        let key = page.to_string();
        if present(&map, &key) {
            map.delete(&key)?;
            return Ok(Outcome::Changed);
        }
        if !self.contains(page) {
            return Err(ModelError::PageNotFound(page));
        }
        Ok(Outcome::Unchanged)
    }

    /// Drops the favourites of pages deleted for good, in the same commit as the delete.
    pub(super) fn forget_favourites(&mut self, pages: &[PageId]) -> Result<(), ModelError> {
        let map = self.favourites_map();
        for page in pages {
            let key = page.to_string();
            if present(&map, &key) {
                map.delete(&key)?;
            }
        }
        Ok(())
    }

    fn favourites_map(&self) -> LoroMap {
        self.doc.get_map(FAVOURITES)
    }

    /// Every entry this version understands. Others (a key that isn't a page ID, a value in a shape
    /// a newer version chose) are skipped here but stay in the document (rule 5).
    fn favourite_entries(&self) -> Vec<(PageId, i64)> {
        let map = self.favourites_map();
        let mut entries = Vec::new();
        map.for_each(|key, value| {
            if let (Ok(page), ValueOrContainer::Value(LoroValue::I64(position))) =
                (key.parse::<PageId>(), value)
            {
                entries.push((page, position));
            }
        });
        entries
    }
}

/// Whether `key` holds anything, in any shape (a deleted key may read as null).
fn present(map: &LoroMap, key: &str) -> bool {
    !matches!(
        map.get(key),
        None | Some(ValueOrContainer::Value(LoroValue::Null))
    )
}

/// `page`'s position, if it is starred with a value this version understands.
fn position(map: &LoroMap, page: PageId) -> Option<i64> {
    match map.get(&page.to_string()) {
        Some(ValueOrContainer::Value(LoroValue::I64(position))) => Some(position),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
