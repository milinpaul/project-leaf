//! Applying workspace commands.
//!
//! Every command follows the same three steps: check everything that could fail, write to Loro, then
//! commit once. Checking first means a rejected command leaves no half-written changes behind,
//! which would otherwise be swept into the next command's commit.

use leaf_core::{DatabaseId, PageId};
use loro::{LoroValue, TreeParentId};

use super::{Workspace, undo};
use crate::page_meta::keys;
use crate::{ModelError, Outcome, WorkspaceCommand};

impl Workspace {
    /// Applies one command as exactly one Loro commit. On error, nothing is written.
    ///
    /// A command that would change nothing (renaming to the same title, moving a page to where it
    /// already is) writes nothing and reports [`Outcome::Unchanged`], so it never leaves an empty
    /// undo step behind.
    pub fn apply(&mut self, command: WorkspaceCommand) -> Result<Outcome, ModelError> {
        let before = self.doc.oplog_vv();
        let outcome = match command {
            WorkspaceCommand::CreatePage {
                parent,
                index,
                title,
                created_at,
            } => Outcome::PageCreated(self.create_page(parent, index, &title, created_at)?),
            WorkspaceCommand::RenamePage { page, title } => {
                let node = self.node(page)?;
                if self.page(page)?.title != title {
                    self.tree.get_meta(node)?.insert(keys::TITLE, title)?;
                }
                Outcome::Changed
            }
            WorkspaceCommand::SetPageIcon { page, icon } => {
                if icon.as_deref() == Some("") {
                    return Err(ModelError::EmptyIcon(page));
                }
                let node = self.node(page)?;
                if self.page(page)?.icon != icon {
                    let meta = self.tree.get_meta(node)?;
                    match icon {
                        Some(icon) => meta.insert(keys::ICON, icon)?,
                        None => meta.delete(keys::ICON)?,
                    }
                }
                Outcome::Changed
            }
            WorkspaceCommand::SetPageCover { page, cover } => {
                if cover.as_deref() == Some("") {
                    return Err(ModelError::EmptyCover(page));
                }
                let node = self.node(page)?;
                if self.page(page)?.cover != cover {
                    let meta = self.tree.get_meta(node)?;
                    match cover {
                        Some(cover) => meta.insert(keys::COVER, cover)?,
                        None => meta.delete(keys::COVER)?,
                    }
                }
                Outcome::Changed
            }
            WorkspaceCommand::SetPageFullWidth { page, full_width } => {
                let node = self.node(page)?;
                if self.page(page)?.full_width != full_width {
                    let meta = self.tree.get_meta(node)?;
                    // Off is the default, so it is stored as no key at all, like a missing cover.
                    if full_width {
                        meta.insert(keys::FULL_WIDTH, true)?;
                    } else {
                        meta.delete(keys::FULL_WIDTH)?;
                    }
                }
                Outcome::Changed
            }
            WorkspaceCommand::MovePage {
                page,
                new_parent,
                index,
            } => self.move_page(page, new_parent, index)?,
            WorkspaceCommand::TrashPage { page, at } => self.trash_page(page, at)?,
            WorkspaceCommand::MarkEdited { page, at } => {
                let node = self.node(page)?;
                if self.page(page)?.edited_at != at {
                    self.tree
                        .get_meta(node)?
                        .insert(keys::EDITED_AT, LoroValue::I64(at))?;
                    self.doc.set_next_commit_origin(undo::EDITED_ORIGIN);
                }
                Outcome::Changed
            }
            WorkspaceCommand::RestorePage { page } => self.restore_page(page)?,
            WorkspaceCommand::CreateDatabasePage {
                parent,
                index,
                title,
                created_at,
            } => self.create_database_page(parent, index, &title, created_at)?,
            WorkspaceCommand::CreateRowPage {
                database_page,
                created_at,
            } => self.create_row_page(database_page, created_at)?,
            WorkspaceCommand::DeletePagePermanently { page } => self.delete_page(page)?,
            WorkspaceCommand::AddFavourite { page } => self.add_favourite(page)?,
            WorkspaceCommand::RemoveFavourite { page } => self.remove_favourite(page)?,
        };
        self.doc.commit();
        // Whether anything was recorded decides it, so `Changed` always means exactly one undo step.
        if outcome == Outcome::Changed && self.doc.oplog_vv() == before {
            return Ok(Outcome::Unchanged);
        }
        Ok(outcome)
    }

    /// Applies `command` like [`Workspace::apply`], but outside undo: for bookkeeping that follows another
    /// document's own undo step, such as a database row's page being created, trashed or restored with the row
    /// (E09-S03). Undo and redo never revisit it; the other document's undo is the step the person sees.
    pub fn apply_unrecorded(&mut self, command: WorkspaceCommand) -> Result<Outcome, ModelError> {
        self.doc.set_next_commit_origin(undo::UNRECORDED_ORIGIN);
        let outcome = self.apply(command);
        // A refused command commits nothing; don't leave the origin for the next one.
        self.doc.set_next_commit_origin("");
        outcome
    }

    fn create_database_page(
        &mut self,
        parent: Option<PageId>,
        index: usize,
        title: &str,
        created_at: i64,
    ) -> Result<Outcome, ModelError> {
        let page = self.create_page(parent, index, title, created_at)?;
        let database = DatabaseId::new();
        self.tree
            .get_meta(self.node(page)?)?
            .insert(keys::DATABASE, database.to_string())?;
        Ok(Outcome::DatabasePageCreated { page, database })
    }

    /// An untitled page at the end of the database page's children, marked as a row of its database.
    fn create_row_page(&mut self, database_page: PageId, created_at: i64) -> Result<Outcome, ModelError> {
        let database = self
            .page(database_page)?
            .database
            .ok_or(ModelError::NotADatabasePage(database_page))?;
        let node = self.node(database_page)?;
        let index = self.tree.children_num(Some(node)).unwrap_or(0);
        let page = self.create_page(Some(database_page), index, "", created_at)?;
        self.tree
            .get_meta(self.node(page)?)?
            .insert(keys::ROW_OF, database.to_string())?;
        Ok(Outcome::PageCreated(page))
    }

    fn create_page(
        &mut self,
        parent: Option<PageId>,
        index: usize,
        title: &str,
        created_at: i64,
    ) -> Result<PageId, ModelError> {
        let parent_node = parent.map(|p| self.node(p)).transpose()?;
        if let (Some(parent), Some(node)) = (parent, parent_node)
            && self.in_trash(node)?
        {
            return Err(ModelError::PageInTrash(parent));
        }
        let len = self.tree.children_num(parent_node).unwrap_or(0);
        if index > len {
            return Err(ModelError::IndexOutOfRange { index, len });
        }
        let page = PageId::new();
        let node = self.tree.create_at(parent_node, index)?;
        let meta = self.tree.get_meta(node)?;
        meta.insert(keys::ID, page.to_string())?;
        meta.insert(keys::TITLE, title)?;
        meta.insert(keys::CREATED_AT, LoroValue::I64(created_at))?;
        meta.insert(keys::EDITED_AT, LoroValue::I64(created_at))?;
        self.index.insert(page, node);
        Ok(page)
    }

    fn move_page(
        &mut self,
        page: PageId,
        new_parent: Option<PageId>,
        index: usize,
    ) -> Result<Outcome, ModelError> {
        let node = self.node(page)?;
        let parent_node = new_parent.map(|p| self.node(p)).transpose()?;
        if self.in_trash(node)? {
            return Err(ModelError::PageInTrash(page));
        }
        if let (Some(target), Some(target_node)) = (new_parent, parent_node)
            && self.in_trash(target_node)?
        {
            return Err(ModelError::PageInTrash(target));
        }
        // Walk up from the destination: meeting the page on the way means it would move into itself.
        let mut cursor = parent_node;
        while let Some(current) = cursor {
            if current == node {
                let new_parent = new_parent.unwrap_or(page);
                return Err(ModelError::MoveIntoOwnSubtree { page, new_parent });
            }
            cursor = match self.tree.parent(current) {
                Some(TreeParentId::Node(up)) => Some(up),
                _ => None,
            };
        }
        let siblings = self.tree.children(parent_node).unwrap_or_default();
        let len = siblings.iter().filter(|&&sibling| sibling != node).count();
        if index > len {
            return Err(ModelError::IndexOutOfRange { index, len });
        }
        if siblings.get(index) == Some(&node) {
            return Ok(Outcome::Changed); // already there; apply reports it as unchanged
        }
        self.tree.mov_to(node, parent_node, index)?;
        Ok(Outcome::Changed)
    }
}

#[cfg(test)]
mod cover_tests;
#[cfg(test)]
mod database_tests;
#[cfg(test)]
mod edited_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod width_tests;
