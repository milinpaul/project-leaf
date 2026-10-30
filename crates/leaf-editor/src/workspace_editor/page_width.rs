//! A page's width (E18-S06): the reading column, or the page area's whole width, chosen per page in
//! the `⋯` menu and saved with the page.

use leaf_core::PageId;
use leaf_model::WorkspaceCommand;

use super::{EditorError, WorkspaceEditor};

impl WorkspaceEditor {
    /// Whether `page`'s title and blocks use the page area's whole width.
    pub fn is_full_width(&self, page: PageId) -> Result<bool, EditorError> {
        Ok(self.workspace.page(page)?.full_width)
    }

    /// Switches `page` to the whole width (`true`) or the reading column. Does nothing when it already
    /// is, so no empty undo step is left behind. It doesn't count as an edit: the page's words are
    /// the same, so its edited time stays.
    pub fn set_full_width(&mut self, page: PageId, on: bool) -> Result<(), EditorError> {
        self.workspace
            .apply(WorkspaceCommand::SetPageFullWidth { page, full_width: on })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
