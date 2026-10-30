//! Databases in the workspace (Phase 1, E09-S03): database pages, and rows that are pages.
//!
//! A database's own document decides which rows exist (ADR 0066). Each row is also a page in the workspace,
//! under the database's page and left out of the sidebar, and its trash state follows the row:
//! - adding a row creates its page and the row;
//! - deleting a row deletes the row and trashes its page;
//! - undo and redo act on the database alone, and the row pages are then brought in line by
//!   [`WorkspaceEditor::reconcile_rows`].
//!
//! The row pages' part is applied outside the workspace's undo ([`Workspace::apply_unrecorded`]), so each of these
//! is one undo step, the database's. Restoring a row page from the Trash restores its row, where it was.
//!
//! [`Workspace::apply_unrecorded`]: leaf_model::Workspace::apply_unrecorded

use leaf_core::{DatabaseId, PageId};
use leaf_model::{Database, DatabaseCommand, ModelError, Outcome, WorkspaceCommand};

use super::{EditorError, WorkspaceEditor};

impl WorkspaceEditor {
    /// Adds an empty database page at the end of `parent`'s pages (or the top level), with its database
    /// document ready, and returns both IDs. One undo step in the workspace, like a new page.
    pub fn new_database_page(
        &mut self,
        parent: Option<PageId>,
        now_ms: i64,
    ) -> Result<(PageId, DatabaseId), EditorError> {
        let index = match parent {
            Some(parent) => self.workspace.children(parent)?.len(),
            None => self.workspace.root_pages()?.len(),
        };
        let Outcome::DatabasePageCreated { page, database } =
            self.workspace.apply(WorkspaceCommand::CreateDatabasePage {
                parent,
                index,
                title: String::new(),
                created_at: now_ms,
            })?
        else {
            unreachable!("CreateDatabasePage always reports the page it created");
        };
        self.databases.insert(database, Database::new());
        Ok((page, database))
    }

    /// Hands over a database as loaded from disk. One already in memory keeps its live copy (it may have newer
    /// edits).
    pub fn insert_database(&mut self, id: DatabaseId, database: Database) {
        self.databases.entry(id).or_insert(database);
    }

    /// A loaded database, read-only.
    #[must_use]
    pub fn database(&self, id: DatabaseId) -> Option<&Database> {
        self.databases.get(&id)
    }

    /// Every loaded database, for saving.
    pub fn loaded_databases(&self) -> impl Iterator<Item = (DatabaseId, &Database)> {
        self.databases.iter().map(|(id, db)| (*id, db))
    }

    /// The database `page` is, when it is a database page.
    pub fn database_of(&self, page: PageId) -> Result<DatabaseId, EditorError> {
        self.workspace
            .page(page)?
            .database
            .ok_or_else(|| ModelError::NotADatabasePage(page).into())
    }

    fn loaded_mut(&mut self, id: DatabaseId) -> Result<&mut Database, EditorError> {
        self.databases
            .get_mut(&id)
            .ok_or(EditorError::DatabaseNotLoaded(id))
    }

    /// Adds a row at position `index` of the database `database_page` is: its page (untitled, under the
    /// database page, out of the sidebar) and the row. One undo step, the database's. Returns the row's page.
    pub fn add_row(
        &mut self,
        database_page: PageId,
        index: usize,
        now_ms: i64,
    ) -> Result<PageId, EditorError> {
        let id = self.database_of(database_page)?;
        let rows = self.loaded_mut(id)?.rows().len();
        if index > rows {
            return Err(ModelError::IndexOutOfRange { index, len: rows }.into());
        }
        let Outcome::PageCreated(row) = self.workspace.apply_unrecorded(WorkspaceCommand::CreateRowPage {
            database_page,
            created_at: now_ms,
        })?
        else {
            unreachable!("CreateRowPage always reports the page it created");
        };
        self.loaded_mut(id)?
            .apply(DatabaseCommand::AddRow { row, index })?;
        Ok(row)
    }

    /// Deletes `row` from the database `database_page` is and trashes its page. One undo step, the database's.
    pub fn delete_row(&mut self, database_page: PageId, row: PageId, now_ms: i64) -> Result<(), EditorError> {
        let id = self.database_of(database_page)?;
        self.loaded_mut(id)?.apply(DatabaseCommand::DeleteRow { row })?;
        self.reconcile_rows(id, now_ms)
    }

    /// Undoes the last change to the database `database_page` is, then brings its row pages in line. Returns
    /// `false` when there was nothing to undo.
    pub fn undo_database(&mut self, database_page: PageId, now_ms: i64) -> Result<bool, EditorError> {
        let id = self.database_of(database_page)?;
        let changed = self.loaded_mut(id)?.undo()?;
        self.reconcile_rows(id, now_ms)?;
        Ok(changed)
    }

    /// Redoes the last undone change to the database `database_page` is, like [`WorkspaceEditor::undo_database`].
    pub fn redo_database(&mut self, database_page: PageId, now_ms: i64) -> Result<bool, EditorError> {
        let id = self.database_of(database_page)?;
        let changed = self.loaded_mut(id)?.redo()?;
        self.reconcile_rows(id, now_ms)?;
        Ok(changed)
    }

    /// Makes each row page's trash state match its row: a page whose row is gone goes to the trash (at
    /// `now_ms`), and a trashed page whose row is back comes out. Outside the workspace's undo. Also needed after
    /// changes from another device are merged into the database (Phase 2).
    pub fn reconcile_rows(&mut self, id: DatabaseId, now_ms: i64) -> Result<(), EditorError> {
        let Some(database) = self.databases.get(&id) else {
            return Err(EditorError::DatabaseNotLoaded(id));
        };
        let mut to_trash = Vec::new();
        let mut to_restore = Vec::new();
        for page in self.workspace.row_pages(id)? {
            let live = database.contains_row(page);
            if live && self.workspace.page(page)?.trashed_at.is_some() {
                to_restore.push(page);
            } else if !live && !self.workspace.is_in_trash(page)? {
                to_trash.push(page);
            }
        }
        for page in to_trash {
            self.workspace
                .apply_unrecorded(WorkspaceCommand::TrashPage { page, at: now_ms })?;
        }
        for page in to_restore {
            self.workspace
                .apply_unrecorded(WorkspaceCommand::RestorePage { page })?;
        }
        Ok(())
    }

    /// When `page` is a row page coming out of the Trash, its row comes back too, where it was. Needs its database
    /// loaded; called before the page is restored, so a refusal changes nothing.
    pub(super) fn restore_row_of(&mut self, page: PageId) -> Result<(), EditorError> {
        let Some(id) = self.workspace.page(page)?.row_of else {
            return Ok(());
        };
        let database = self.loaded_mut(id)?;
        if database.contains_row(page) {
            return Ok(());
        }
        match database.apply(DatabaseCommand::RestoreRow { row: page }) {
            // Never a row here (its add was undone): it comes back at the end.
            Err(ModelError::RowNotFound(_)) => {
                let index = database.rows().len();
                database.apply(DatabaseCommand::AddRow { row: page, index })?;
            }
            other => {
                other?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
