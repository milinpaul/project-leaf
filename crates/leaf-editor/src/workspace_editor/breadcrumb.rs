//! Where the open page sits, for the top bar's breadcrumb (E18-S03): its parent pages from the top,
//! then the page itself, each with its icon and title. A long path is shortened in the middle, as
//! Notion does, so the first page and the nearest two stay visible.

use leaf_core::PageId;

use super::{EditorError, UNTITLED, WorkspaceEditor};

/// Up to this many steps are shown in full; a longer path keeps the first and the last two.
pub const MAX_CRUMBS: usize = 4;

/// One step of the breadcrumb.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crumb {
    /// A page: clicking it opens it.
    Page {
        /// The page.
        page: PageId,
        /// Its title, or [`UNTITLED`] when it has none.
        label: String,
        /// Its emoji icon, if it has one.
        icon: Option<String>,
    },
    /// The pages left out of a long path, drawn as "…".
    Gap,
}

impl WorkspaceEditor {
    /// The breadcrumb for `page`: its ancestors from the top level down, then the page. Paths longer
    /// than [`MAX_CRUMBS`] keep the first page, a [`Crumb::Gap`] and the last two.
    pub fn breadcrumb(&self, page: PageId) -> Result<Vec<Crumb>, EditorError> {
        let mut path = vec![page];
        let mut at = page;
        // Bounded by the page count, so a damaged tree can't loop for ever.
        while path.len() <= self.workspace.len() {
            match self.workspace.parent(at)? {
                Some(parent) => {
                    path.push(parent);
                    at = parent;
                }
                None => break,
            }
        }
        path.reverse();
        let shown: Vec<Option<PageId>> = if path.len() > MAX_CRUMBS {
            let tail = &path[path.len() - 2..];
            [Some(path[0]), None]
                .into_iter()
                .chain(tail.iter().copied().map(Some))
                .collect()
        } else {
            path.into_iter().map(Some).collect()
        };
        shown
            .into_iter()
            .map(|step| match step {
                None => Ok(Crumb::Gap),
                Some(page) => {
                    let meta = self.workspace.page(page)?;
                    Ok(Crumb::Page {
                        page,
                        label: if meta.title.is_empty() {
                            UNTITLED.to_owned()
                        } else {
                            meta.title
                        },
                        icon: meta.icon,
                    })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
