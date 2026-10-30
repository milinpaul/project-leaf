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

/// How many of the best matching blocks are considered when picking each page's best
/// [`BLOCKS_PER_PAGE`]. Picking happens before excerpts are made, so a page with hundreds of
/// matching blocks can't crowd the others out of the results.
const CANDIDATE_BLOCKS: i64 = 2_000;

/// How many blocks get an excerpt: enough for [`MAX_PAGES`] pages of [`BLOCKS_PER_PAGE`] each,
/// with room for pages whose title matched as well. Excerpts are the costly part of reading a row.
const EXCERPT_BLOCKS: i64 = 300;

/// At most this many matching blocks are ranked: the most recently indexed ones. Ranking scores
/// every match, about a millisecond per thousand, so a word in nearly every block (a single letter
/// typed so far) would blow the 50 ms budget (E11-S07, ADR 0058). Below this, every match is
/// ranked; above it, an older block can be missed, which only happens for words so common that
/// the search bar couldn't show all their pages anyway. Titles are always all searched.
pub const RANKED_BLOCKS: i64 = 10_000;

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

/// The matching blocks to show, best first, in three steps so the costly parts only touch the
/// rows that are kept:
/// - `ranked`: the best [`CANDIDATE_BLOCKS`], scored in FTS5 alone among the newest
///   [`RANKED_BLOCKS`] matches (those at or above the rowid the inner subquery finds; rowids only
///   grow, so a page just re-indexed is always among them);
/// - `kept`: each page's best [`BLOCKS_PER_PAGE`] of those, at most [`EXCERPT_BLOCKS`];
/// - the outer query makes their excerpts. The `+` keeps SQLite from looking each kept row up
///   through FTS5 one at a time, which re-runs the whole match per row; one pass is far cheaper.
const BLOCK_QUERY: &str = "
WITH ranked AS (
  SELECT rowid AS id, bm25(blocks_fts) AS score
  FROM blocks_fts
  WHERE blocks_fts MATCH ?1
    AND rowid >= coalesce((SELECT rowid FROM blocks_fts WHERE blocks_fts MATCH ?1
                           ORDER BY rowid DESC LIMIT 1 OFFSET ?3), 0)
  ORDER BY score
  LIMIT ?4),
kept AS (
  SELECT id FROM (
    SELECT r.id, r.score, row_number() OVER (PARTITION BY b.page_id ORDER BY r.score, r.id) AS nth
    FROM ranked r JOIN blocks b ON b.id = r.id)
  WHERE nth <= ?5
  ORDER BY score
  LIMIT ?6)
SELECT b.page_id, b.block_id, b.kind, snippet(blocks_fts, 0, char(1), char(2), '…', ?2),
       bm25(blocks_fts), p.title, p.saved_at
FROM blocks_fts JOIN blocks b ON b.id = blocks_fts.rowid JOIN pages p ON p.page_id = b.page_id
WHERE blocks_fts MATCH ?1 AND +blocks_fts.rowid IN kept
ORDER BY bm25(blocks_fts), b.id";

/// Searches `conn` for `query`. Every word must match, as a prefix, ignoring case and accents.
/// Pages whose title matches come first; then best match first; then most recently edited. Only
/// the newest [`RANKED_BLOCKS`] matching blocks are ranked. An empty or punctuation-only query
/// returns nothing.
pub(crate) fn search(conn: &Connection, query: &str) -> Result<Vec<PageHit>, SearchError> {
    let Some(expression) = match_expression(query) else {
        return Ok(Vec::new());
    };
    let mut groups: HashMap<PageId, Group> = HashMap::new();
    add_title_matches(conn, &expression, &mut groups)?;
    add_block_matches(conn, &expression, &mut groups)?;

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

/// Every page whose title matches, with the title highlighted.
fn add_title_matches(
    conn: &Connection,
    expression: &str,
    groups: &mut HashMap<PageId, Group>,
) -> Result<(), SearchError> {
    let mut titles = conn.prepare_cached(
        "SELECT p.page_id, highlight(pages_fts, 0, char(1), char(2)), bm25(pages_fts), p.saved_at
         FROM pages_fts JOIN pages p ON p.id = pages_fts.rowid
         WHERE pages_fts MATCH ?1",
    )?;
    let rows = titles.query_map([expression], |row| {
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
    Ok(())
}

/// The best matching blocks ([`BLOCK_QUERY`]), added under their pages.
fn add_block_matches(
    conn: &Connection,
    expression: &str,
    groups: &mut HashMap<PageId, Group>,
) -> Result<(), SearchError> {
    let mut blocks = conn.prepare_cached(BLOCK_QUERY)?;
    let per_page = i64::try_from(BLOCKS_PER_PAGE).unwrap_or(i64::MAX);
    let limits = params![
        expression,
        EXCERPT_WORDS,
        RANKED_BLOCKS - 1,
        CANDIDATE_BLOCKS,
        per_page,
        EXCERPT_BLOCKS
    ];
    let rows = blocks.query_map(limits, |row| {
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
    Ok(())
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
