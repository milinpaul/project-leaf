//! Turning title matches and full-text hits into the rows the search bar draws, grouped by page.
//!
//! Order: pages whose title matched in the index, then pages whose title matched the quick switcher's
//! looser matching (letters in order, as you type), then pages that matched only in their text. Under
//! each page come its matching blocks. Pages not in the workspace's live tree (in the trash, or
//! deleted since the index was written) are left out.

use std::collections::{HashMap, HashSet};
use std::ops::Range;

use leaf_core::{BlockId, PageId};

use super::{Destination, Marked, SearchHit};
use crate::{MAX_SWITCHER_RESULTS, QuickSwitcher, SwitcherResult};

/// A page in the results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRow {
    /// The page.
    pub page: PageId,
    /// Its title, or "Untitled".
    pub label: String,
    /// Its emoji, if any.
    pub icon: Option<String>,
    /// The pages it lives in, as "Projects / Leaf app"; empty at the top level.
    pub path: String,
    /// The parts of `label` that matched, as byte ranges, for bold text.
    pub highlights: Vec<Range<usize>>,
    /// Where choosing the row scrolls to: the best matching block when only the text matched, else
    /// the top of the page.
    pub opens_at: Option<BlockId>,
}

/// A matching block, shown under its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptRow {
    /// The page the block is in.
    pub page: PageId,
    /// The block.
    pub block: BlockId,
    /// A short part of its text around the match ("…" where it was cut).
    pub text: String,
    /// The parts of `text` that matched, as byte ranges, for highlighted text.
    pub highlights: Vec<Range<usize>>,
}

/// One row of the search bar. Every row can be highlighted and chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchRow {
    /// A page: icon, title and where it lives.
    Page(PageRow),
    /// A matching block under the page above it.
    Excerpt(ExcerptRow),
}

impl SearchRow {
    /// Where choosing this row goes.
    #[must_use]
    pub fn destination(&self) -> Destination {
        match self {
            Self::Page(row) => Destination {
                page: row.page,
                block: row.opens_at,
            },
            Self::Excerpt(row) => Destination {
                page: row.page,
                block: Some(row.block),
            },
        }
    }
}

/// The empty query's rows: recent pages, then the rest (the switcher's own list).
pub(super) fn recent(titles: &QuickSwitcher) -> Vec<SearchRow> {
    titles
        .results()
        .iter()
        .map(|result| page_row(result.clone(), None))
        .collect()
}

/// The rows for a query: title matches alone until the full-text `hits` arrive, then both.
pub(super) fn results<'a>(titles: &QuickSwitcher, hits: Option<&'a [SearchHit]>) -> Vec<SearchRow> {
    let Some(hits) = hits else {
        return recent(titles); // the switcher's list is its title matches once a query is typed
    };
    let by_page: HashMap<PageId, &SearchHit> = hits.iter().map(|hit| (hit.page, hit)).collect();
    let mut listed: HashSet<PageId> = HashSet::new();
    let mut pages: Vec<(SwitcherResult, Option<&'a SearchHit>)> = Vec::new();
    let mut add = |result: SwitcherResult, hit: Option<&'a SearchHit>| {
        if listed.insert(result.page) {
            pages.push((result, hit));
        }
    };
    for hit in hits.iter().filter(|hit| hit.title_matched) {
        if let Some(result) = titles.describe(hit.page) {
            add(result, Some(hit));
        }
    }
    let fuzzy: HashMap<PageId, &SwitcherResult> = titles
        .results()
        .iter()
        .map(|result| (result.page, result))
        .collect();
    for result in titles.results() {
        add(result.clone(), by_page.get(&result.page).copied());
    }
    for hit in hits.iter().filter(|hit| !hit.title_matched) {
        if let Some(result) = titles.describe(hit.page) {
            add(result, Some(hit));
        }
    }
    let mut rows = Vec::new();
    for (mut result, hit) in pages.into_iter().take(MAX_SWITCHER_RESULTS) {
        if let Some(found) = fuzzy.get(&result.page) {
            result.highlights.clone_from(&found.highlights);
        }
        let Some(hit) = hit else {
            rows.push(page_row(result, None));
            continue;
        };
        if hit.title_matched && hit.title.text == result.label {
            result.highlights = byte_ranges(&hit.title);
        }
        let title_matched = hit.title_matched || !result.highlights.is_empty();
        let opens_at = if title_matched {
            None
        } else {
            hit.blocks.first().map(|block| block.block)
        };
        rows.push(page_row(result, opens_at));
        rows.extend(hit.blocks.iter().map(|block| {
            SearchRow::Excerpt(ExcerptRow {
                page: hit.page,
                block: block.block,
                text: block.excerpt.text.clone(),
                highlights: byte_ranges(&block.excerpt),
            })
        }));
    }
    rows
}

fn page_row(result: SwitcherResult, opens_at: Option<BlockId>) -> SearchRow {
    SearchRow::Page(PageRow {
        page: result.page,
        label: result.label,
        icon: result.icon,
        path: result.path,
        highlights: result.highlights,
        opens_at,
    })
}

/// The index marks characters; drawing needs bytes. Ranges past the end are clipped.
fn byte_ranges(marked: &Marked) -> Vec<Range<usize>> {
    let len = marked.text.len();
    let byte_at = |chars: usize| marked.text.char_indices().nth(chars).map_or(len, |(at, _)| at);
    marked
        .ranges
        .iter()
        .map(|range| byte_at(range.start)..byte_at(range.end))
        .filter(|range| !range.is_empty())
        .collect()
}

#[cfg(test)]
mod tests;
