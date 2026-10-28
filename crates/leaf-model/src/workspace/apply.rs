//! Applying workspace commands.
//!
//! Every command follows the same three steps: check everything that could fail, write to Loro, then
//! commit once. Checking first means a rejected command leaves no half-written changes behind,
//! which would otherwise be swept into the next command's commit.

use leaf_core::PageId;
use loro::{LoroValue, TreeParentId};

use super::Workspace;
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
            } => self.create_page(parent, index, &title, created_at)?,
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
            WorkspaceCommand::MovePage {
                page,
                new_parent,
                index,
            } => self.move_page(page, new_parent, index)?,
            WorkspaceCommand::TrashPage { page, at } => self.trash_page(page, at)?,
            WorkspaceCommand::RestorePage { page } => self.restore_page(page)?,
            WorkspaceCommand::DeletePagePermanently { page } => self.delete_page(page)?,
        };
        self.doc.commit();
        // Whether anything was recorded decides it, so `Changed` always means exactly one undo step.
        if outcome == Outcome::Changed && self.doc.oplog_vv() == before {
            return Ok(Outcome::Unchanged);
        }
        Ok(outcome)
    }

    fn create_page(
        &mut self,
        parent: Option<PageId>,
        index: usize,
        title: &str,
        created_at: i64,
    ) -> Result<Outcome, ModelError> {
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
        self.index.insert(page, node);
        Ok(Outcome::PageCreated(page))
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
mod tests;
