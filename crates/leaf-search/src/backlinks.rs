//! Which pages link to a page (E10-S06, ADR-071): read from the `links` table, which indexing fills from
//! each page's text, so it is as current as search and rebuilt with it.

use std::str::FromStr;

use leaf_core::{BlockId, PageId};
use rusqlite::Connection;

use crate::SearchError;

/// One block that links to the page asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlink {
    /// The linking page.
    pub page: PageId,
    /// Its title as indexed ("Untitled" when blank).
    pub title: String,
    /// The block with the link, to open the page scrolled to it.
    pub block: BlockId,
    /// The words just before the link ("…" in front where they were cut).
    pub before: String,
    /// The words just after it ("…" at the end where they were cut).
    pub after: String,
}

/// The backlinks of `target`: linking pages by title (case ignored), each page's blocks in reading order.
/// A page's links to itself are left out.
pub(crate) fn backlinks(conn: &Connection, target: PageId) -> Result<Vec<Backlink>, SearchError> {
    let target = target.to_string();
    let mut stmt = conn.prepare_cached(
        "SELECT links.page_id, pages.title, links.block_id, links.before, links.after
         FROM links JOIN pages ON pages.page_id = links.page_id
         WHERE links.target_id = ?1 AND links.page_id != ?1
         ORDER BY pages.title COLLATE NOCASE, links.page_id, links.position",
    )?;
    let rows = stmt.query_map([&target], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (page, title, block, before, after) = row?;
        // Rows this crate didn't write are skipped rather than failing the whole list.
        if let (Ok(page), Ok(block)) = (PageId::from_str(&page), BlockId::from_str(&block)) {
            out.push(Backlink {
                page,
                title,
                block,
                before,
                after,
            });
        }
    }
    Ok(out)
}
