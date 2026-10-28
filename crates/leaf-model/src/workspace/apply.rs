//! Applying workspace commands.
//!
//! Every command follows the same three steps: check everything that could fail, write to Loro, then
//! commit once. Checking first means a rejected command leaves no half-written changes behind,
//! which would otherwise be swept into the next command's commit.

use leaf_core::PageId;
use loro::LoroValue;

use super::Workspace;
use crate::page_meta::keys;
use crate::{ModelError, Outcome, WorkspaceCommand};

impl Workspace {
    /// Applies one command as exactly one Loro commit. On error, nothing is written.
    pub fn apply(&mut self, command: WorkspaceCommand) -> Result<Outcome, ModelError> {
        let outcome = match command {
            WorkspaceCommand::CreatePage {
                parent,
                index,
                title,
                created_at,
            } => self.create_page(parent, index, &title, created_at)?,
            WorkspaceCommand::RenamePage { page, title } => {
                let meta = self.tree.get_meta(self.node(page)?)?;
                meta.insert(keys::TITLE, title)?;
                Outcome::Changed
            }
            WorkspaceCommand::SetPageIcon { page, icon } => {
                if icon.as_deref() == Some("") {
                    return Err(ModelError::EmptyIcon(page));
                }
                let meta = self.tree.get_meta(self.node(page)?)?;
                match icon {
                    Some(icon) => meta.insert(keys::ICON, icon)?,
                    None => meta.delete(keys::ICON)?,
                }
                Outcome::Changed
            }
        };
        self.doc.commit();
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
}

#[cfg(test)]
mod tests;
