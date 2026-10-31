//! Inline databases, the workspace's side (E09-S09, ADR-066 answer 4).
//!
//! The database of a block on a page is a database page filed under that page, so the sidebar lists it there;
//! its rows stay out of the sidebar as always. The page's undo history holds the block, not the database page,
//! so making the database page and trashing or restoring it all happen outside the workspace's undo
//! ([`Workspace::apply_unrecorded`]), each following what the block did:
//! - `/database` makes the database page and puts its block where it was typed;
//! - when the block leaves the page (deleted, cut, undone), its database goes to the Trash, rows and all;
//! - when it comes back (undo, redo), so does its database, if it went to the Trash on its own.
//!
//! [`Workspace::apply_unrecorded`]: leaf_model::Workspace::apply_unrecorded

use leaf_core::PageId;
use leaf_model::WorkspaceCommand;

use super::{EditorError, WorkspaceEditor};

impl WorkspaceEditor {
    /// Makes the database the open page asked for (the slash menu's "Database"), under the open page, and puts
    /// its block where `/database` was typed. Returns the database's page, or `None` when nothing was asked for.
    pub fn make_requested_database(&mut self, now_ms: i64) -> Result<Option<PageId>, EditorError> {
        let Some(host) = self.selected else {
            return Ok(None);
        };
        let Some(at) = self
            .pages
            .get_mut(&host)
            .map(crate::PageEditor::take_database_request)
            .transpose()?
            .flatten()
        else {
            return Ok(None);
        };
        let (database, _) = self.create_database_page(Some(host), now_ms, false)?;
        if let Some(editor) = self.pages.get_mut(&host) {
            editor.insert_inline_database(at, database)?;
        }
        // Shown in the sidebar under the page it is on.
        self.expanded.insert(host);
        Ok(Some(database))
    }

    /// Brings the open page's inline databases in line with their blocks: a database whose block left the page
    /// goes to the Trash (at `now_ms`), and one whose block came back comes out. Call after each change to the
    /// page; the first call for a page only takes note of its databases. Returns whether any page moved.
    pub fn sync_inline_databases(&mut self, now_ms: i64) -> Result<bool, EditorError> {
        let Some(host) = self.selected else {
            return Ok(false);
        };
        let Some(editor) = self.pages.get_mut(&host) else {
            return Ok(false);
        };
        let changes = editor.inline_database_changes()?;
        let mut moved = false;
        for page in changes.gone {
            // Already in the Trash (with the page it was on, say), or deleted for good: nothing to do.
            if self.workspace.contains(page) && !self.workspace.is_in_trash(page)? {
                self.workspace
                    .apply_unrecorded(WorkspaceCommand::TrashPage { page, at: now_ms })?;
                moved = true;
            }
        }
        for page in changes.back {
            if self.workspace.contains(page) && self.workspace.page(page)?.trashed_at.is_some() {
                self.workspace
                    .apply_unrecorded(WorkspaceCommand::RestorePage { page })?;
                moved = true;
            }
        }
        Ok(moved)
    }
}

#[cfg(test)]
mod tests;
