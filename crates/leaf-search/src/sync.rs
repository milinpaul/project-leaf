//! Bringing the index in line with the saved documents.
//!
//! Instead of the editor reporting each change, the indexer compares what is saved (each page's
//! [`PageVersion`], the workspace's titles and trash) with what it indexed, and fixes the
//! difference (ADR 0023). That one comparison handles edits, renames, trash, restore and permanent
//! delete alike, and also edits saved just before a crash, which nobody got to report.

use std::collections::HashMap;

use leaf_core::PageId;
use leaf_model::Workspace;

use crate::extract::title;
use crate::{PageSource, PageText, PageVersion, SearchError, SearchIndex, page_text};

/// How a reconcile or rebuild ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finished {
    /// The index matches the saved documents.
    Done,
    /// Asked to stop part-way (the app is closing). A rebuild stays incomplete and reruns next time.
    Stopped,
}

/// Keeps one [`SearchIndex`] in step with one [`PageSource`]. Single-threaded and clock-free, so
/// every case is testable; [`crate::SearchWorker`] runs it on a background thread.
pub struct Syncer<S> {
    index: SearchIndex,
    source: S,
    /// The last workspace document read, with the version it was read at, so it is only reloaded
    /// when titles or the page tree changed.
    workspace: Option<(Option<PageVersion>, Workspace)>,
}

impl<S: PageSource> Syncer<S> {
    /// Starts keeping `index` in step with `source`.
    pub fn new(index: SearchIndex, source: S) -> Self {
        Self {
            index,
            source,
            workspace: None,
        }
    }

    /// The index, for searching it.
    pub fn index(&self) -> &SearchIndex {
        &self.index
    }

    /// Re-indexes every page that changed since it was indexed and removes pages that are gone or
    /// in the trash. Pages that didn't change aren't read at all.
    pub fn reconcile(&mut self, stop: &dyn Fn() -> bool) -> Result<Finished, SearchError> {
        let versions = self.source.versions().map_err(SearchError::Source)?;
        let current = matches!(&self.workspace, Some((seen, _)) if *seen == versions.workspace);
        if !current {
            let workspace = self.source.workspace().map_err(SearchError::Source)?;
            self.workspace = Some((versions.workspace, workspace));
        }
        let Some((_, workspace)) = &self.workspace else {
            return Ok(Finished::Done); // just filled in above
        };
        let wanted = searchable(workspace)?;
        let indexed = self.index.indexed_pages()?;

        for (&page, wanted_title) in &wanted {
            let version = versions.pages.get(&page).copied().unwrap_or_default();
            if let Some(have) = indexed.get(&page)
                && have.title == *wanted_title
                && have.version == version
            {
                continue;
            }
            if stop() {
                return Ok(Finished::Stopped);
            }
            let saved = if versions.pages.contains_key(&page) {
                self.source.page(page).map_err(SearchError::Source)?
            } else {
                None
            };
            let text = match saved {
                Some(doc) => page_text(workspace, page, &doc)?,
                None => Some(PageText {
                    page,
                    title: wanted_title.clone(),
                    blocks: Vec::new(),
                }),
            };
            if let Some(text) = text {
                self.index.index_page(&text, version)?;
            }
        }
        for page in indexed.keys().filter(|page| !wanted.contains_key(page)) {
            self.index.remove_page(*page)?;
        }
        Ok(Finished::Done)
    }

    /// Empties the index and indexes every page again, then records that it finished. Stopping
    /// part-way leaves it marked incomplete, so the next open rebuilds again.
    pub fn rebuild(&mut self, stop: &dyn Fn() -> bool) -> Result<Finished, SearchError> {
        self.index.clear()?;
        let finished = self.reconcile(stop)?;
        if finished == Finished::Done {
            self.index.mark_complete()?;
        }
        Ok(finished)
    }

    /// Throws the index file away and starts again empty, after an error. A rebuild is due next.
    pub fn recreate(&mut self) -> Result<(), SearchError> {
        self.index.recreate()
    }
}

/// Every page search should find (not in the trash), with the title it is indexed under.
fn searchable(workspace: &Workspace) -> Result<HashMap<PageId, String>, SearchError> {
    let mut pages = HashMap::new();
    let mut stack = workspace.root_pages()?;
    while let Some(page) = stack.pop() {
        pages.insert(page, title(workspace, page)?);
        stack.extend(workspace.children(page)?);
    }
    Ok(pages)
}
