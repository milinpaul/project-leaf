//! The search bar (Cmd/Ctrl+K, Cmd/Ctrl+P): one box that finds pages by title and by the text inside
//! them, with results grouped by page.
//!
//! Titles are matched here, at once, as in the quick switcher. Full-text results come from the search
//! index, which this crate can't reach (it must stay free of SQLite and wasm-ok): the bar says when a
//! search is due ([`SearchBar::due`]), the app runs it and hands the hits back as plain data
//! ([`SearchBar::deliver`]). Typing waits for a short pause before searching; the policy is clock-free,
//! like `Autosave`, so the app passes the time in and tests use a fake clock. Every query gets a rising
//! id, and hits for anything but the latest are ignored, so a slow search never overwrites a newer one.

use std::ops::Range;

use leaf_core::{BlockId, PageId};
use leaf_model::Workspace;

use crate::{EditorError, QuickSwitcher};

mod rows;
mod shortcut;

pub use rows::{ExcerptRow, PageRow, SearchRow};
pub use shortcut::{Shortcut, ShortcutAction, shortcut_action};

/// How long typing must pause, in milliseconds, before the full-text search runs. Short enough to feel
/// instant, long enough not to search for every letter of a word typed quickly.
pub const SEARCH_DELAY_MS: i64 = 150;

/// Text with the parts that matched marked, as the search index returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marked {
    /// The text.
    pub text: String,
    /// Matched parts, as ranges of characters (not bytes) in `text`, in order.
    pub ranges: Vec<Range<usize>>,
}

/// One matching block under a page in the full-text results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMatch {
    /// The block, so choosing it can scroll there.
    pub block: BlockId,
    /// A short part of the block's text around the match.
    pub excerpt: Marked,
}

/// One page in the full-text results, best first. The app converts the search index's results into
/// these, so this crate doesn't depend on the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// The page.
    pub page: PageId,
    /// Its title as indexed, marked where it matched.
    pub title: Marked,
    /// Whether the title itself matched; such pages are listed first.
    pub title_matched: bool,
    /// The best matching blocks; empty if only the title matched.
    pub blocks: Vec<BlockMatch>,
}

/// A full-text search the app should run now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Hand this back with the hits ([`SearchBar::deliver`]).
    pub id: u64,
    /// The query as typed.
    pub query: String,
}

/// Where choosing a row goes: a page, scrolled to a block if one matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination {
    /// The page to open.
    pub page: PageId,
    /// The block to scroll to; `None` for the top of the page.
    pub block: Option<BlockId>,
}

/// A key the search bar handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKey {
    /// Highlight the row above (the last from the first).
    Up,
    /// Highlight the row below (the first from the last).
    Down,
    /// Open the highlighted row.
    Enter,
    /// Close the search bar.
    Escape,
}

/// What a key asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOutcome {
    /// Nothing beyond redrawing the bar.
    Nothing,
    /// Close the bar and open this page.
    Open(Destination),
    /// Close the bar.
    Close,
}

/// The search bar's state while it is open.
#[derive(Debug, Clone)]
pub struct SearchBar {
    /// Live pages (titles, icons, breadcrumbs) and the title matching.
    titles: QuickSwitcher,
    delay: i64,
    query: String,
    /// The current query's id; hits for any other are stale.
    latest: u64,
    /// When the full-text search for the current query falls due, if it hasn't been asked for yet.
    due_at: Option<i64>,
    /// The full-text hits for the current query, once they arrive.
    hits: Option<Vec<SearchHit>>,
    rows: Vec<SearchRow>,
    selected: usize,
}

impl SearchBar {
    /// Opens the bar over `workspace`'s live pages, waiting [`SEARCH_DELAY_MS`] after typing. With
    /// nothing typed it lists `recent` pages (most recent first), then the rest in sidebar order.
    pub fn open(workspace: &Workspace, recent: &[PageId]) -> Result<Self, EditorError> {
        Self::with_delay(workspace, recent, SEARCH_DELAY_MS)
    }

    /// [`SearchBar::open`] with another pause before searching, in milliseconds.
    pub fn with_delay(workspace: &Workspace, recent: &[PageId], delay: i64) -> Result<Self, EditorError> {
        let mut bar = Self {
            titles: QuickSwitcher::open(workspace, recent)?,
            delay,
            query: String::new(),
            latest: 0,
            due_at: None,
            hits: None,
            rows: Vec::new(),
            selected: 0,
        };
        bar.rebuild();
        Ok(bar)
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The query changed at `now`: titles are matched at once, and the full-text search falls due once
    /// typing pauses. The highlight goes back to the first row.
    pub fn set_query(&mut self, query: &str, now: i64) {
        if self.query == query {
            return;
        }
        query.clone_into(&mut self.query);
        self.latest = self.latest.wrapping_add(1);
        self.hits = None;
        self.titles.set_query(query);
        self.due_at = (!blank(query)).then_some(now.saturating_add(self.delay));
        self.selected = 0;
        self.rebuild();
    }

    /// When the app's timer should next call [`SearchBar::due`]; `None` when no search is waiting.
    #[must_use]
    pub fn next_deadline(&self) -> Option<i64> {
        self.due_at
    }

    /// The full-text search to run, if typing has paused long enough by `now`. Each query is asked
    /// for once.
    pub fn due(&mut self, now: i64) -> Option<SearchRequest> {
        let due_at = self.due_at?;
        if now < due_at {
            return None;
        }
        self.due_at = None;
        Some(SearchRequest {
            id: self.latest,
            query: self.query.clone(),
        })
    }

    /// Hits for search `id` arrived. Ignored (returns `false`) unless `id` is the current query's, so
    /// a slow search can't replace a newer one. The highlight stays on the same row if it is still
    /// listed.
    pub fn deliver(&mut self, id: u64, hits: Vec<SearchHit>) -> bool {
        if id != self.latest || blank(&self.query) {
            return false;
        }
        let kept = self.selected_row().map(SearchRow::destination);
        self.hits = Some(hits);
        self.rebuild();
        self.selected = kept
            .and_then(|kept| self.rows.iter().position(|row| row.destination() == kept))
            .unwrap_or(0);
        true
    }

    /// Whether full-text results for the current query are still to come (for a quiet "Searching…").
    #[must_use]
    pub fn searching(&self) -> bool {
        !blank(&self.query) && self.hits.is_none()
    }

    /// The rows to draw, top to bottom.
    #[must_use]
    pub fn rows(&self) -> &[SearchRow] {
        &self.rows
    }

    /// The highlighted row, if there are any rows.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        (!self.rows.is_empty()).then_some(self.selected)
    }

    /// Highlights row `index` (the pointer moved over it). Ignored if there is no such row.
    pub fn select(&mut self, index: usize) {
        if index < self.rows.len() {
            self.selected = index;
        }
    }

    /// Up, Down, Enter or Escape.
    pub fn press(&mut self, key: SearchKey) -> SearchOutcome {
        let count = self.rows.len();
        match key {
            SearchKey::Up if count > 0 => {
                self.selected = self
                    .selected
                    .checked_sub(1)
                    .map_or(count.saturating_sub(1), |prev| prev.min(count.saturating_sub(1)));
                SearchOutcome::Nothing
            }
            SearchKey::Down if count > 0 => {
                self.selected = self.selected.saturating_add(1).checked_rem(count).unwrap_or(0);
                SearchOutcome::Nothing
            }
            SearchKey::Enter => self.selected_row().map_or(SearchOutcome::Nothing, |row| {
                SearchOutcome::Open(row.destination())
            }),
            SearchKey::Escape => SearchOutcome::Close,
            SearchKey::Up | SearchKey::Down => SearchOutcome::Nothing,
        }
    }

    fn selected_row(&self) -> Option<&SearchRow> {
        self.rows.get(self.selected)
    }

    fn rebuild(&mut self) {
        self.rows = if blank(&self.query) {
            rows::recent(&self.titles)
        } else {
            rows::results(&self.titles, self.hits.as_deref())
        };
    }
}

/// Nothing but spaces: show recent pages and don't search.
fn blank(query: &str) -> bool {
    query.trim().is_empty()
}

#[cfg(test)]
mod tests;
