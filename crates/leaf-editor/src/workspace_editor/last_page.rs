//! Reopening the page that was open when the workspace was last closed on this computer (E06-S19).
//! The UI keeps which page that was in the workspace's view state; the editor decides whether it can
//! still be opened.

use leaf_core::PageId;

use super::WorkspaceEditor;

impl WorkspaceEditor {
    /// Opens `page` again if it is still in the workspace and not in the trash, and says whether it
    /// did. A page that is gone or trashed is skipped, so the workspace opens as if none was saved.
    pub fn restore_open_page(&mut self, page: Option<PageId>) -> bool {
        let Some(page) = page else {
            return false;
        };
        let usable = self.workspace.contains(page) && matches!(self.workspace.is_in_trash(page), Ok(false));
        usable && self.select(page).is_ok()
    }
}

#[cfg(test)]
mod tests;
