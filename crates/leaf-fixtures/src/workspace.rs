//! A big workspace: thousands of pages in a tree a few levels deep (some pages with emoji icons),
//! and optionally every page filled with blocks.

use leaf_core::PageId;
use leaf_model::{Outcome, PageDoc, Workspace, WorkspaceCommand};

use crate::page::fill_page;
use crate::words::{ICONS, words};
use crate::{FixtureError, Rng};

/// How deep the page tree goes at most.
const MAX_DEPTH: usize = 6;

/// When fixture pages say they were created (2026-01-01, Unix milliseconds): fixed, so fixtures
/// don't depend on the time of day.
pub(crate) const CREATED_AT: i64 = 1_767_225_600_000;

/// A workspace and the blocks of each of its pages.
pub struct Fixture {
    /// The page tree, titles and icons.
    pub workspace: Workspace,
    /// Every page's blocks, in the order the pages were made (parents before their children).
    pub pages: Vec<(PageId, PageDoc)>,
}

impl Fixture {
    /// How many blocks the fixture holds in all.
    #[must_use]
    pub fn block_count(&self) -> usize {
        self.pages.iter().map(|(_, page)| page.len()).sum()
    }
}

/// A workspace with `pages` pages in a tree, titled and some with icons, outline fixed by `seed`;
/// the pages have no blocks. For the sidebar, quick switcher and page-tree budgets.
pub fn page_tree(seed: u64, pages: usize) -> Result<Workspace, FixtureError> {
    Ok(build(seed, pages, 0)?.workspace)
}

/// A workspace with `pages` pages in a tree, each holding `blocks_per_page` blocks, outline fixed
/// by `seed`. With [`crate::BIG_WORKSPACE_PAGES`] and [`crate::BLOCKS_PER_PAGE`]: the search
/// budget's 5,000 pages and 100,000 blocks.
pub fn big_workspace(seed: u64, pages: usize, blocks_per_page: usize) -> Result<Fixture, FixtureError> {
    build(seed, pages, blocks_per_page)
}

fn build(seed: u64, pages: usize, blocks_per_page: usize) -> Result<Fixture, FixtureError> {
    let mut rng = Rng::new(seed);
    let mut workspace = Workspace::new();
    // Every page made so far with its depth, so a new one can go under any page not too deep.
    let mut made: Vec<(PageId, usize)> = Vec::with_capacity(pages);
    let mut docs = Vec::with_capacity(pages);
    for _ in 0..pages {
        let (parent, depth) = if made.is_empty() || rng.chance(15) {
            (None, 0)
        } else {
            // Mostly near recent pages, so sections fill up like real notes do.
            let recent = made.len().min(60);
            let (page, depth) = made[made.len() - 1 - rng.below(recent)];
            if depth < MAX_DEPTH {
                (Some(page), depth + 1)
            } else {
                (None, 0)
            }
        };
        let index = workspace.child_slots(parent)?.len();
        let count = rng.between(1, 5);
        let title = words(&mut rng, count);
        let Outcome::PageCreated(page) = workspace.apply(WorkspaceCommand::CreatePage {
            parent,
            index,
            title,
            created_at: CREATED_AT,
        })?
        else {
            return Err(FixtureError::Unexpected("no page for CreatePage".into()));
        };
        if rng.chance(15) {
            let icon = rng.pick(ICONS).map(|icon| (*icon).to_owned());
            workspace.apply(WorkspaceCommand::SetPageIcon { page, icon })?;
        }
        made.push((page, depth));
        let page_seed = rng.next_u64();
        if blocks_per_page > 0 {
            let mut doc = PageDoc::new();
            fill_page(&mut doc, page_seed, blocks_per_page)?;
            docs.push((page, doc));
        }
    }
    Ok(Fixture {
        workspace,
        pages: docs,
    })
}
