//! When to save: a clock-free policy that turns "something changed" into timely, cheap saves.
//!
//! The product promises every edit is on disk within a second, with no save button. Saving after
//! every keystroke would be wasteful, so changes are collected and each document is saved at most
//! [`DEFAULT_AUTOSAVE_MS`] after its *first* unsaved change. Counting from the first change (a
//! throttle, not a debounce) means steady typing can never put a save off.
//!
//! The policy never reads the clock or starts threads: the app passes the time in and runs the
//! timer, so tests drive it with a fake clock.

use std::collections::HashMap;

use crate::{DocKey, Savable, Store, StoreError};

/// How long, in milliseconds, a change may wait before it is saved (`BLOCK_MODEL.md`).
pub const DEFAULT_AUTOSAVE_MS: i64 = 300;

/// Which documents have unsaved changes, and since when.
///
/// The app calls [`Autosave::changed`] after each command, sets a timer for
/// [`Autosave::next_deadline`], and saves what [`Autosave::due`] lists. On window blur and on quit
/// it saves everything in [`Autosave::pending`].
#[derive(Debug, Clone)]
pub struct Autosave {
    delay: i64,
    /// First unsaved change per document, in Unix milliseconds.
    dirty: HashMap<DocKey, i64>,
}

impl Default for Autosave {
    fn default() -> Self {
        Self::new(DEFAULT_AUTOSAVE_MS)
    }
}

impl Autosave {
    /// A policy that saves each document `delay` milliseconds after its first unsaved change.
    #[must_use]
    pub fn new(delay: i64) -> Self {
        Self {
            delay,
            dirty: HashMap::new(),
        }
    }

    /// Notes that `key` changed at `now`. Later changes before the save keep the first time, so
    /// the deadline never moves later.
    pub fn changed(&mut self, key: DocKey, now: i64) {
        self.dirty.entry(key).or_insert(now);
    }

    /// When the next document falls due, or `None` when everything is saved: when to fire the timer.
    #[must_use]
    pub fn next_deadline(&self) -> Option<i64> {
        self.dirty.values().min().map(|first| first + self.delay)
    }

    /// The documents whose changes have waited long enough to be saved at `now`.
    #[must_use]
    pub fn due(&self, now: i64) -> Vec<DocKey> {
        self.dirty
            .iter()
            .filter(|&(_, &first)| now - first >= self.delay)
            .map(|(&key, _)| key)
            .collect()
    }

    /// Every document with unsaved changes, due or not: what to save on blur and quit.
    #[must_use]
    pub fn pending(&self) -> Vec<DocKey> {
        self.dirty.keys().copied().collect()
    }

    /// Saves `doc` under `key` now, due or not, and forgets it once the write succeeded.
    /// Returns whether anything was written. On error the document stays pending, so it is retried.
    pub fn save<D: Savable + ?Sized>(
        &mut self,
        store: &mut Store,
        key: DocKey,
        doc: &D,
        now: i64,
    ) -> Result<bool, StoreError> {
        let wrote = store.save(key, doc, now)?;
        self.dirty.remove(&key);
        Ok(wrote)
    }

    /// One beat of the app's timer: notices which of `docs` have unsaved changes (so a change can't
    /// be missed even if nobody called [`Autosave::changed`]) and saves those that are due.
    /// Returns how many documents were written.
    pub fn tick(
        &mut self,
        store: &mut Store,
        docs: &[(DocKey, &dyn Savable)],
        now: i64,
    ) -> Result<usize, StoreError> {
        self.notice(docs, now);
        let due = self.due(now);
        self.save_listed(store, docs, &due, now)
    }

    /// Saves every document with unsaved changes right away: on window blur and on quit.
    pub fn flush(
        &mut self,
        store: &mut Store,
        docs: &[(DocKey, &dyn Savable)],
        now: i64,
    ) -> Result<usize, StoreError> {
        self.notice(docs, now);
        let pending = self.pending();
        self.save_listed(store, docs, &pending, now)
    }

    fn notice(&mut self, docs: &[(DocKey, &dyn Savable)], now: i64) {
        for (key, doc) in docs {
            if doc.has_unsaved_changes() {
                self.changed(*key, now);
            }
        }
    }

    /// Saves `keys`, stopping at the first failure (the rest stay pending). A pending key with no
    /// document any more is forgotten.
    fn save_listed(
        &mut self,
        store: &mut Store,
        docs: &[(DocKey, &dyn Savable)],
        keys: &[DocKey],
        now: i64,
    ) -> Result<usize, StoreError> {
        let mut wrote = 0;
        for key in keys {
            match docs.iter().find(|(k, _)| k == key) {
                Some((_, doc)) => wrote += usize::from(self.save(store, *key, *doc, now)?),
                None => self.forget(*key),
            }
        }
        Ok(wrote)
    }

    /// Forgets `key` without saving, for a document that no longer exists (a permanently deleted
    /// page), so the timer doesn't keep firing for it.
    pub fn forget(&mut self, key: DocKey) {
        self.dirty.remove(&key);
    }
}

#[cfg(test)]
mod tests;
