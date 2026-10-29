//! Running a search: ranked results grouped by page, with highlighted excerpts.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::str::FromStr;

use leaf_core::{BlockId, PageId};
use rusqlite::{Connection, OpenFlags, params};

use crate::query::match_expression;
use crate::{INDEX_FILE, SEARCH_DIR, SearchError};

/// At most this many pages are returned: more than a search bar can usefully show.
pub const MAX_PAGES: usize = 50;

/// At most this many matching blocks are shown under each page, best first.
pub const BLOCKS_PER_PAGE: usize = 3;

/// How many matching blocks are read before grouping. Bounds the work for a very common word
/// while leaving room for [`MAX_PAGES`] pages.
const MAX_BLOCK_ROWS: i64 = 1000;

/// Roughly how many words an excerpt shows around the match.
const EXCERPT_WORDS: i64 = 16;

/// Markers FTS5 puts around matched words. Control characters, which typed text doesn't contain,
/// so they can be found and removed again without ambiguity.
const OPEN: char = '\u{1}';
const CLOSE: char = '\u{2}';

/// Text with the parts that matched the search marked, for drawing them highlighted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlighted {
    /// The text to show.
    pub text: String,
    /// Matched parts, as ranges of characters (not bytes) in `text`, in order.
    pub ranges: Vec<Range<usize>>,
}

/// One block that matched, shown under its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockHit {
    /// The block, so choosing the result can scroll to it.
    pub block: BlockId,
    /// The block's kind name (`paragraph`, `code`, ...).
    pub kind: String,
    /// A short part of the block's text around the match ("…" where it was cut).
    pub excerpt: Highlighted,
}

/// One page in the results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageHit {
    /// The page to open.
    pub page: PageId,
    /// Its title, highlighted where it matched.
    pub title: Highlighted,
    /// Whether the title itself matched. Such pages are listed first.
    pub title_matched: bool,
    /// The best matching blocks, at most [`BLOCKS_PER_PAGE`]; empty if only the title matched.
    pub blocks: Vec<BlockHit>,
}

/// A read-only connection to a workspace's index, for searching while the index is being written.
///
/// The search bar keeps one of these. The index file uses write-ahead logging, so searches never
/// wait for, or block, re-indexing.
#[derive(Debug)]
pub struct Searcher {
    conn: Connection,
}

impl Searcher {
    /// Opens the index of the workspace at `workspace_root` for reading. Fails if there is no
    /// index yet; the background indexer creates it when the workspace opens.
    pub fn open(workspace_root: &Path) -> Result<Self, SearchError> {
        let path = workspace_root.join(SEARCH_DIR).join(INDEX_FILE);
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        Ok(Self {
            conn: Connection::open_with_flags(path, flags)?,
        })
    }

    /// Searches for `query` as typed. See [`crate::SearchIndex::search`].
    pub fn search(&self, query: &str) -> Result<Vec<PageHit>, SearchError> {
        search(&self.conn, query)
    }
}

/// A page being assembled from title and block matches.
struct Group {
    hit: PageHit,
    /// BM25 of the title match, or of the best block if the title didn't match; lower is better.
    score: f64,
    saved_at: i64,
}

/// Searches `conn` for `query`. Every word must match, as a prefix, ignoring case and accents.
/// Pages whose title matches come first; then best match first; then most recently edited.
/// An empty or punctuation-only query returns nothing.
pub(crate) fn search(conn: &Connection, query: &str) -> Result<Vec<PageHit>, SearchError> {
    let Some(expression) = match_expression(query) else {
        return Ok(Vec::new());
    };
    let mut groups: HashMap<PageId, Group> = HashMap::new();

    let mut titles = conn.prepare_cached(
        "SELECT p.page_id, highlight(pages_fts, 0, char(1), char(2)), bm25(pages_fts), p.saved_at
         FROM pages_fts JOIN pages p ON p.id = pages_fts.rowid
         WHERE pages_fts MATCH ?1",
    )?;
    let rows = titles.query_map([&expression], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    for row in rows {
        let (id, title, score, saved_at) = row?;
        let Ok(page) = PageId::from_str(&id) else { continue };
        let hit = PageHit {
            page,
            title: highlighted(&title),
            title_matched: true,
            blocks: Vec::new(),
        };
        groups.insert(page, Group { hit, score, saved_at });
    }

    let mut blocks = conn.prepare_cached(
        "SELECT b.page_id, b.block_id, b.kind, snippet(blocks_fts, 0, char(1), char(2), '…', ?2),
                bm25(blocks_fts), p.title, p.saved_at
         FROM blocks_fts JOIN blocks b ON b.id = blocks_fts.rowid JOIN pages p ON p.page_id = b.page_id
         WHERE blocks_fts MATCH ?1
         ORDER BY bm25(blocks_fts)
         LIMIT ?3",
    )?;
    let rows = blocks.query_map(params![expression, EXCERPT_WORDS, MAX_BLOCK_ROWS], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, f64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    for row in rows {
        let (page_id, block_id, kind, excerpt, score, title, saved_at) = row?;
        let (Ok(page), Ok(block)) = (PageId::from_str(&page_id), BlockId::from_str(&block_id)) else {
            continue;
        };
        // Rows arrive best first, so the first block of a page sets a body-only page's score.
        let group = groups.entry(page).or_insert_with(|| Group {
            hit: PageHit {
                page,
                title: plain(title),
                title_matched: false,
                blocks: Vec::new(),
            },
            score,
            saved_at,
        });
        if group.hit.blocks.len() < BLOCKS_PER_PAGE {
            group.hit.blocks.push(BlockHit {
                block,
                kind,
                excerpt: highlighted(&excerpt),
            });
        }
    }

    let mut groups: Vec<Group> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        b.hit
            .title_matched
            .cmp(&a.hit.title_matched)
            .then(a.score.partial_cmp(&b.score).unwrap_or(Ordering::Equal))
            .then(b.saved_at.cmp(&a.saved_at))
            .then(a.hit.page.cmp(&b.hit.page))
    });
    Ok(groups
        .into_iter()
        .take(MAX_PAGES)
        .map(|group| group.hit)
        .collect())
}

/// Text with no matches marked.
fn plain(text: String) -> Highlighted {
    Highlighted {
        text,
        ranges: Vec::new(),
    }
}

/// Removes the [`OPEN`]/[`CLOSE`] markers, recording where they were in characters.
fn highlighted(marked: &str) -> Highlighted {
    let mut text = String::with_capacity(marked.len());
    let mut ranges = Vec::new();
    let (mut chars, mut start) = (0, None);
    for c in marked.chars() {
        match c {
            OPEN => start = Some(chars),
            CLOSE => ranges.extend(start.take().map(|start| start..chars)),
            _ => {
                text.push(c);
                chars += 1;
            }
        }
    }
    Highlighted { text, ranges }
}

#[cfg(test)]
mod tests;
