//! Favourite pages, as the sidebar's Favourites section and the top bar's star show them (ADR 0060).

use leaf_core::PageId;
use leaf_model::WorkspaceCommand;

use super::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};

impl WorkspaceEditor {
    /// The favourites as sidebar rows, in the order they were starred. Pages in the trash are left
    /// out until restored. Rows are flat (depth 0) and show whether the page has children, but
    /// never list them: the page tree below does that.
    pub fn favourites(&self) -> Result<Vec<SidebarItem>, EditorError> {
        let mut items = Vec::new();
        for page in self.workspace.favourites()? {
            let meta = self.workspace.page(page)?;
            items.push(SidebarItem {
                page,
                label: if meta.title.is_empty() {
                    UNTITLED.to_owned()
                } else {
                    meta.title
                },
                icon: meta.icon,
                selected: self.selected == Some(page),
                depth: 0,
                has_children: !self.workspace.children(page)?.is_empty(),
                expanded: false,
            });
        }
        Ok(items)
    }

    /// Whether `page` is starred, for the top bar's star and the page menu's label.
    #[must_use]
    pub fn is_favourite(&self, page: PageId) -> bool {
        self.workspace.is_favourite(page)
    }

    /// Stars (`true`) or unstars `page`. Does nothing when it already is (or isn't) a favourite, so
    /// no empty undo step is left behind. Starring a page in the trash is refused.
    pub fn set_favourite(&mut self, page: PageId, on: bool) -> Result<(), EditorError> {
        let command = if on {
            WorkspaceCommand::AddFavourite { page }
        } else {
            WorkspaceCommand::RemoveFavourite { page }
        };
        self.workspace.apply(command)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
