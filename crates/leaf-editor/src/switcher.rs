//! The quick switcher (Cmd/Ctrl+P): type part of a page's title, pick it from a short list, go there.
//!
//! Opening takes a snapshot of the live pages (titles, icons and where each one lives) in one walk of
//! the tree, so typing only re-scores titles and never reads the document. With nothing typed it lists
//! recently opened pages first. Everything is keyboard driven: the UI forwards typing, Up/Down and Enter.
//! The search bar ([`crate::SearchBar`]) builds on this, adding full-text results.

use std::ops::Range;

use leaf_core::PageId;
use leaf_model::Workspace;

use crate::fuzzy::{Lowered, query_words};
use crate::{EditorError, UNTITLED, Vertical};

/// The most results shown at once; more than a screenful is noise, and typing narrows it.
pub const MAX_SWITCHER_RESULTS: usize = 50;

/// What separates the titles in a breadcrumb.
const PATH_SEPARATOR: &str = " / ";

/// One row of the switcher, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitcherResult {
    /// The page this row opens.
    pub page: PageId,
    /// The title, or [`UNTITLED`].
    pub label: String,
    /// The page's emoji, if any.
    pub icon: Option<String>,
    /// The pages it lives in, outermost first, as "Projects / Leaf app"; empty at the top level.
    pub path: String,
    /// The parts of `label` that matched the query, as byte ranges, for bold text.
    pub highlights: Vec<Range<usize>>,
}

/// A live page as the switcher saw it on opening.
#[derive(Debug, Clone)]
struct Entry {
    page: PageId,
    label: String,
    /// `label` prepared for matching.
    lowered: Lowered,
    icon: Option<String>,
    path: String,
    /// Position in the sidebar's top-to-bottom order, the last tie-break.
    order: usize,
}

/// The switcher's state while it is open.
#[derive(Debug, Clone)]
pub struct QuickSwitcher {
    entries: Vec<Entry>,
    /// Recently opened pages, most recent first, for the empty query.
    recent: Vec<PageId>,
    query: String,
    results: Vec<SwitcherResult>,
    selected: usize,
}

impl QuickSwitcher {
    /// Opens the switcher over `workspace`'s live pages (not those in the trash). `recent` lists recently
    /// opened pages, most recent first; they lead the list until something is typed.
    pub fn open(workspace: &Workspace, recent: &[PageId]) -> Result<Self, EditorError> {
        let mut switcher = Self {
            entries: entries(workspace)?,
            recent: recent.to_vec(),
            query: String::new(),
            results: Vec::new(),
            selected: 0,
        };
        switcher.refresh();
        Ok(switcher)
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Updates the list for a new query; the selection goes back to the best match.
    pub fn set_query(&mut self, query: &str) {
        if self.query != query {
            query.clone_into(&mut self.query);
            self.refresh();
        }
    }

    /// The rows to show, best first.
    #[must_use]
    pub fn results(&self) -> &[SwitcherResult] {
        &self.results
    }

    /// The highlighted row, if there are any rows.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        (!self.results.is_empty()).then_some(self.selected)
    }

    /// Highlights row `index` (the pointer moved over it). Ignored if there is no such row.
    pub fn select(&mut self, index: usize) {
        if index < self.results.len() {
            self.selected = index;
        }
    }

    /// Up or Down: moves the highlight, wrapping from the last row to the first and back.
    pub fn move_selection(&mut self, direction: Vertical) {
        let count = self.results.len();
        if count == 0 {
            return;
        }
        self.selected = match direction {
            Vertical::Up => (self.selected + count - 1) % count,
            Vertical::Down => (self.selected + 1) % count,
        };
    }

    /// The page Enter opens: the highlighted row's.
    #[must_use]
    pub fn chosen(&self) -> Option<PageId> {
        self.results.get(self.selected).map(|result| result.page)
    }

    /// `page`'s row with nothing highlighted, if it is a live page: how the search bar shows a page
    /// that only the full-text index found.
    pub(crate) fn describe(&self, page: PageId) -> Option<SwitcherResult> {
        self.entries
            .iter()
            .find(|entry| entry.page == page)
            .map(|entry| result(entry, Vec::new()))
    }

    fn refresh(&mut self) {
        self.selected = 0;
        self.results = if self.query.trim().is_empty() {
            self.recent_first()
        } else {
            self.matches()
        };
    }

    /// Recent pages, then the rest in sidebar order.
    fn recent_first(&self) -> Vec<SwitcherResult> {
        let recent = self
            .recent
            .iter()
            .filter_map(|page| self.entries.iter().find(|entry| entry.page == *page));
        let rest = self
            .entries
            .iter()
            .filter(|entry| !self.recent.contains(&entry.page));
        recent
            .chain(rest)
            .take(MAX_SWITCHER_RESULTS)
            .map(|entry| result(entry, Vec::new()))
            .collect()
    }

    /// Pages whose titles match, best first; ties go to the shorter title, then sidebar order.
    fn matches(&self) -> Vec<SwitcherResult> {
        let words = query_words(&self.query);
        let mut scored: Vec<(u32, &Entry, Vec<Range<usize>>)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                entry
                    .lowered
                    .matches(&words)
                    .map(|found| (found.score, entry, found.ranges))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(a.1.label.len().cmp(&b.1.label.len()))
                .then(a.1.order.cmp(&b.1.order))
        });
        scored
            .into_iter()
            .take(MAX_SWITCHER_RESULTS)
            .map(|(_, entry, ranges)| result(entry, ranges))
            .collect()
    }
}

fn result(entry: &Entry, highlights: Vec<Range<usize>>) -> SwitcherResult {
    SwitcherResult {
        page: entry.page,
        label: entry.label.clone(),
        icon: entry.icon.clone(),
        path: entry.path.clone(),
        highlights,
    }
}

/// Every live page with its breadcrumb, in sidebar order (depth-first, an explicit stack so a deep
/// tree can't overflow the call stack).
fn entries(workspace: &Workspace) -> Result<Vec<Entry>, EditorError> {
    let mut entries = Vec::with_capacity(workspace.len());
    let mut stack: Vec<(PageId, String)> = workspace
        .root_pages()?
        .into_iter()
        .rev()
        .map(|page| (page, String::new()))
        .collect();
    while let Some((page, path)) = stack.pop() {
        let meta = workspace.page(page)?;
        let label = if meta.title.is_empty() {
            UNTITLED.to_owned()
        } else {
            meta.title
        };
        let inside = if path.is_empty() {
            label.clone()
        } else {
            format!("{path}{PATH_SEPARATOR}{label}")
        };
        stack.extend(
            workspace
                .children(page)?
                .into_iter()
                .rev()
                .map(|child| (child, inside.clone())),
        );
        let order = entries.len();
        entries.push(Entry {
            page,
            lowered: Lowered::new(&label),
            label,
            icon: meta.icon,
            path,
            order,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests;
