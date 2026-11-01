//! What the workspace tells page links (E10-S03, ADR-071): what a link to a page shows, and which page a
//! pasted `[[Title]]` means. A link to a block (E10-S08) also reads the block's words from its page,
//! when that page is loaded.

use std::collections::HashMap;

use leaf_core::{BlockId, PageId};
use leaf_model::PAGE_LINK;

use super::{EditorError, WorkspaceEditor};
use crate::page_link::{LinkState, LinkTarget, LinkedBlock};

impl WorkspaceEditor {
    /// What a link to `page` shows: its current title and icon, or that it is in the trash or gone.
    #[must_use]
    pub fn link_target(&self, page: PageId) -> LinkTarget {
        let workspace = self.workspace();
        let Ok(meta) = workspace.page(page) else {
            return LinkTarget::missing();
        };
        let state = if workspace.is_in_trash(page).unwrap_or(false) {
            LinkState::Trashed
        } else {
            LinkState::Live
        };
        LinkTarget {
            title: meta.title,
            icon: meta.icon,
            state,
            block: None,
        }
    }

    /// What a link to `page`, or to `block` inside it, shows. The block's words come from the page if
    /// it is loaded (the UI loads the pages an open page's block links point at); a loaded page without
    /// the block makes the link [`LinkState::BlockGone`].
    #[must_use]
    pub fn link_to(&self, page: PageId, block: Option<BlockId>) -> LinkTarget {
        let mut target = self.link_target(page);
        let Some(block) = block else {
            return target;
        };
        let known = match self.page_editor(page) {
            None => LinkedBlock::Unknown,
            Some(editor) => match editor.doc().text(block) {
                Ok(spans) => LinkedBlock::Words(
                    spans
                        .unwrap_or_default()
                        .iter()
                        .flat_map(|span| span.text.chars())
                        .filter(|&c| c != PAGE_LINK)
                        .collect(),
                ),
                Err(_) => LinkedBlock::Gone,
            },
        };
        if known == LinkedBlock::Gone && target.state == LinkState::Live {
            target.state = LinkState::BlockGone;
        }
        target.block = Some(known);
        target
    }

    /// The pages the open page's links to blocks point at that aren't loaded yet: the UI loads them, so
    /// each link can show its block's words. Pages not in the workspace are left out (nothing to load).
    #[must_use]
    pub fn block_link_pages_to_load(&self) -> Vec<PageId> {
        let Some(open) = self.selected().and_then(|page| self.page_editor(page)) else {
            return Vec::new();
        };
        let mut pages: Vec<PageId> = Vec::new();
        for (_, page, _) in open.doc().block_links() {
            if !pages.contains(&page) && self.page_editor(page).is_none() && self.workspace().contains(page) {
                pages.push(page);
            }
        }
        pages
    }

    /// Each title held by exactly one page outside the trash, with that page: what a pasted `[[Title]]`
    /// links to. A title two pages share links to neither (there's no telling which was meant).
    pub fn titles_for_links(&self) -> Result<HashMap<String, PageId>, EditorError> {
        let workspace = self.workspace();
        let mut seen: HashMap<String, Option<PageId>> = HashMap::new();
        let mut stack = workspace.root_pages()?;
        while let Some(page) = stack.pop() {
            stack.extend(workspace.children(page)?);
            let title = workspace.page(page)?.title.trim().to_owned();
            if title.is_empty() {
                continue;
            }
            seen.entry(title)
                .and_modify(|found| *found = None)
                .or_insert(Some(page));
        }
        Ok(seen
            .into_iter()
            .filter_map(|(title, page)| page.map(|page| (title, page)))
            .collect())
    }
}

#[cfg(test)]
mod tests;
