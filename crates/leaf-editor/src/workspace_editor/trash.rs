//! The Trash: what is in it, putting pages back, and deleting them for good.
//!
//! Moving a page to the trash is in `tree`. The Trash view lists pages trashed on their own (newest
//! first), each with where it lived and when it was trashed, filtered as the owner types. Deleting
//! for good returns every page that went, so storage can drop their documents.

use std::ops::Range;

use leaf_core::PageId;
use leaf_model::{Outcome, Workspace, WorkspaceCommand};

use super::{EditorError, UNTITLED, WorkspaceEditor};
use crate::fuzzy::{Lowered, query_words};

/// One page in the Trash view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashItem {
    /// The trashed page (everything inside it went with it).
    pub page: PageId,
    /// The title, or [`UNTITLED`].
    pub label: String,
    /// The page's emoji, if any.
    pub icon: Option<String>,
    /// When it was trashed, in Unix milliseconds.
    pub trashed_at: i64,
    /// Where it lived, outermost first ("Projects / Leaf app"); empty for the top level.
    pub path: String,
}

impl WorkspaceEditor {
    /// The pages in the Trash, most recently trashed first.
    pub fn trash_items(&self) -> Result<Vec<TrashItem>, EditorError> {
        let workspace = &self.workspace;
        workspace
            .trash()?
            .into_iter()
            .map(|page| {
                let meta = workspace.page(page)?;
                Ok(TrashItem {
                    page,
                    label: label(meta.title),
                    icon: meta.icon,
                    trashed_at: meta.trashed_at.unwrap_or_default(),
                    path: path(workspace, page)?,
                })
            })
            .collect()
    }

    /// Takes `page` out of the trash, back where it was (or to the end of the top level if the page it
    /// lived in is still in the trash).
    pub fn restore(&mut self, page: PageId) -> Result<(), EditorError> {
        self.workspace.apply(WorkspaceCommand::RestorePage { page })?;
        Ok(())
    }

    /// Deletes `page`, which must be in the trash, and everything inside it, for good. Returns every
    /// deleted page so their documents can be removed from disk. The window forgets them too: their
    /// blocks, expanded state and place in the recent list; if one was open, nothing is open now.
    pub fn delete_forever(&mut self, page: PageId) -> Result<Vec<PageId>, EditorError> {
        let outcome = self
            .workspace
            .apply(WorkspaceCommand::DeletePagePermanently { page })?;
        let Outcome::PagesDeleted(deleted) = outcome else {
            return Ok(Vec::new());
        };
        self.forget(&deleted);
        Ok(deleted)
    }

    /// Deletes everything in the trash for good; see [`WorkspaceEditor::delete_forever`].
    pub fn empty_trash(&mut self) -> Result<Vec<PageId>, EditorError> {
        let mut deleted = Vec::new();
        for page in self.workspace.trash()? {
            // A page trashed on its own inside another trashed page went with that one already.
            if self.workspace.contains(page) {
                deleted.extend(self.delete_forever(page)?);
            }
        }
        Ok(deleted)
    }

    /// Whether the open page is in the trash (opened from the Trash view), so the window shows a
    /// banner offering to restore it.
    pub fn selected_in_trash(&self) -> Result<bool, EditorError> {
        match self.selected {
            Some(page) => Ok(self.workspace.is_in_trash(page)?),
            None => Ok(false),
        }
    }

    fn forget(&mut self, pages: &[PageId]) {
        for page in pages {
            self.pages.remove(page);
            self.expanded.remove(page);
        }
        self.recent.retain(|page| !pages.contains(page));
        if self.selected.is_some_and(|open| pages.contains(&open)) {
            self.selected = None;
        }
    }
}

fn label(title: String) -> String {
    if title.is_empty() {
        UNTITLED.to_owned()
    } else {
        title
    }
}

/// The titles of the pages `page` lives in, outermost first, joined with " / ".
fn path(workspace: &Workspace, page: PageId) -> Result<String, EditorError> {
    let mut titles = Vec::new();
    let mut cursor = workspace.parent(page)?;
    while let Some(parent) = cursor {
        titles.push(label(workspace.page(parent)?.title));
        cursor = workspace.parent(parent)?;
    }
    titles.reverse();
    Ok(titles.join(" / "))
}

/// How long ago something was trashed, in words: "just now", "5 min ago", "3 h ago", "yesterday",
/// "4 days ago", or "6 weeks ago". Times are Unix milliseconds; the app passes `now`.
#[must_use]
pub fn trashed_ago(now: i64, at: i64) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let ago = now.saturating_sub(at).max(0);
    match ago {
        _ if ago < MINUTE => "just now".to_owned(),
        _ if ago < HOUR => format!("{} min ago", ago / MINUTE),
        _ if ago < DAY => format!("{} h ago", ago / HOUR),
        _ if ago < 2 * DAY => "yesterday".to_owned(),
        _ if ago < 14 * DAY => format!("{} days ago", ago / DAY),
        _ => format!("{} weeks ago", ago / (7 * DAY)),
    }
}

/// The Trash view's list: every trashed page, narrowed by a filter typed by the owner.
#[derive(Debug, Clone)]
pub struct TrashList {
    items: Vec<(TrashItem, Lowered)>,
    query: String,
    shown: Vec<(usize, Vec<Range<usize>>)>,
}

impl TrashList {
    /// The list over `items` (from [`WorkspaceEditor::trash_items`]), showing them all.
    #[must_use]
    pub fn new(items: Vec<TrashItem>) -> Self {
        let items: Vec<(TrashItem, Lowered)> = items
            .into_iter()
            .map(|item| {
                let lowered = Lowered::new(&item.label);
                (item, lowered)
            })
            .collect();
        let shown = (0..items.len()).map(|ix| (ix, Vec::new())).collect();
        Self {
            items,
            query: String::new(),
            shown,
        }
    }

    /// Narrows the list to titles matching `query` (as the quick switcher matches), best first.
    pub fn set_query(&mut self, query: &str) {
        query.clone_into(&mut self.query);
        let words = query_words(query);
        if words.is_empty() {
            self.shown = (0..self.items.len()).map(|ix| (ix, Vec::new())).collect();
            return;
        }
        let mut found: Vec<(u32, usize, Vec<Range<usize>>)> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(ix, (_, lowered))| lowered.matches(&words).map(|m| (m.score, ix, m.ranges)))
            .collect();
        found.sort_by_key(|(score, ix, _)| (std::cmp::Reverse(*score), *ix));
        self.shown = found.into_iter().map(|(_, ix, ranges)| (ix, ranges)).collect();
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The pages to show, each with the parts of its title that matched (for bold text).
    pub fn shown(&self) -> impl Iterator<Item = (&TrashItem, &[Range<usize>])> {
        self.shown
            .iter()
            .filter_map(|(ix, ranges)| self.items.get(*ix).map(|(item, _)| (item, ranges.as_slice())))
    }

    /// How many pages are in the trash, filter or not (for "Empty trash (3)").
    #[must_use]
    pub fn total(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests;
