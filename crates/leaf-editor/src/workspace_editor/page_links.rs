//! What the workspace tells page links (E10-S03, ADR-071): what a link to a page shows, and which page a
//! pasted `[[Title]]` means.

use std::collections::HashMap;

use leaf_core::PageId;

use super::{EditorError, WorkspaceEditor};
use crate::page_link::{LinkState, LinkTarget};

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
        }
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
