//! The backlinks list under a page's title (E10-S07, ADR-071): which blocks link to the open page, as
//! found by the search index, shown with each linking page's current icon and title.

use leaf_core::{BlockId, PageId};

use super::WorkspaceEditor;
use crate::UNTITLED;
use crate::page_link::LinkState;

/// One block linking to the page, as the index found it: plain data, so the editor doesn't depend on
/// the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkSource {
    /// The linking page.
    pub page: PageId,
    /// The block with the link.
    pub block: BlockId,
    /// The words just before the link.
    pub before: String,
    /// The words just after it.
    pub after: String,
}

/// One row of the backlinks list, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkRow {
    /// The linking page, to open.
    pub page: PageId,
    /// The block to scroll to.
    pub block: BlockId,
    /// The linking page's icon.
    pub icon: Option<String>,
    /// The linking page's title, or "Untitled".
    pub title: String,
    /// The words before the link.
    pub before: String,
    /// The link as it reads in that page: the open page's icon and title.
    pub link: String,
    /// The words after the link.
    pub after: String,
}

impl WorkspaceEditor {
    /// The backlinks of `target` from what the index found: each with the linking page's current icon
    /// and title, and without pages that are now in the trash or gone (the index may be a moment behind).
    #[must_use]
    pub fn backlinks(&self, target: PageId, found: Vec<BacklinkSource>) -> Vec<BacklinkRow> {
        let link = self.link_target(target).label();
        found
            .into_iter()
            .filter_map(|source| {
                let page = self.link_target(source.page);
                (page.state == LinkState::Live).then(|| BacklinkRow {
                    page: source.page,
                    block: source.block,
                    icon: page.icon,
                    title: if page.title.trim().is_empty() {
                        UNTITLED.to_owned()
                    } else {
                        page.title
                    },
                    before: source.before,
                    link: link.clone(),
                    after: source.after,
                })
            })
            .collect()
    }
}

/// What the line under the title says: "1 backlink", "3 backlinks"; nothing for none.
#[must_use]
pub fn backlinks_label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 backlink".to_owned()),
        n => Some(format!("{n} backlinks")),
    }
}

#[cfg(test)]
mod tests;
