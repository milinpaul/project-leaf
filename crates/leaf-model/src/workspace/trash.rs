//! The trash: soft delete with `trashed_at`, restore, and permanent delete.
//!
//! Trashing never moves a page, so it remembers where it lived. A page counts as "in the trash" when
//! it or any page above it has `trashed_at` set; listings hide it, and commands that would put
//! something into the trash or move it out without restoring are refused.

use leaf_core::PageId;
use loro::{LoroValue, TreeID, TreeParentId};

use super::Workspace;
use crate::page_meta::{keys, value};
use crate::{ModelError, Outcome, PageMeta};

impl Workspace {
    /// Pages trashed on their own (not just hidden because a page above them is), most recently
    /// trashed first. Each one can be restored or deleted for good.
    pub fn trash(&self) -> Result<Vec<PageId>, ModelError> {
        let mut trashed: Vec<PageMeta> = Vec::new();
        for &page in self.index.by_page.keys() {
            let meta = self.page(page)?;
            if meta.trashed_at.is_some() {
                trashed.push(meta);
            }
        }
        // Newest first; ties broken by ID so the order never depends on hash-map iteration.
        trashed.sort_by(|a, b| b.trashed_at.cmp(&a.trashed_at).then(a.id.cmp(&b.id)));
        Ok(trashed.into_iter().map(|meta| meta.id).collect())
    }

    /// Whether `page` is in the trash, directly or because a page above it is.
    pub fn is_in_trash(&self, page: PageId) -> Result<bool, ModelError> {
        self.in_trash(self.node(page)?)
    }

    pub(super) fn trash_page(&mut self, page: PageId, at: i64) -> Result<Outcome, ModelError> {
        let node = self.node(page)?;
        if self.in_trash(node)? {
            return Err(ModelError::PageInTrash(page));
        }
        self.tree
            .get_meta(node)?
            .insert(keys::TRASHED_AT, LoroValue::I64(at))?;
        Ok(Outcome::Changed)
    }

    pub(super) fn restore_page(&mut self, page: PageId) -> Result<Outcome, ModelError> {
        let node = self.node(page)?;
        if !self.is_trashed(node)? {
            return Err(ModelError::NotInTrash(page));
        }
        let parent_in_trash = match self.tree.parent(node) {
            Some(TreeParentId::Node(parent)) => self.in_trash(parent)?,
            _ => false,
        };
        self.tree.get_meta(node)?.delete(keys::TRASHED_AT)?;
        if parent_in_trash {
            // The original parent is itself in the trash: put the page at the end of the top level
            // rather than restoring it somewhere it would still be hidden.
            let end = self.tree.children_num(None::<TreeID>).unwrap_or(0);
            self.tree.mov_to(node, None::<TreeID>, end)?;
        }
        Ok(Outcome::Changed)
    }

    pub(super) fn delete_page(&mut self, page: PageId) -> Result<Outcome, ModelError> {
        let node = self.node(page)?;
        if !self.in_trash(node)? {
            return Err(ModelError::NotInTrash(page));
        }
        let mut deleted = Vec::new();
        self.collect_subtree(node, &mut deleted);
        self.tree.delete(node)?;
        for &gone in &deleted {
            self.index.remove(gone);
        }
        self.forget_favourites(&deleted)?;
        Ok(Outcome::PagesDeleted(deleted))
    }

    /// Whether this node itself carries `trashed_at` (not its ancestors).
    pub(super) fn is_trashed(&self, node: TreeID) -> Result<bool, ModelError> {
        Ok(matches!(
            value(&self.tree.get_meta(node)?, keys::TRASHED_AT),
            Some(LoroValue::I64(_))
        ))
    }

    /// Whether this node or any node above it is trashed.
    pub(super) fn in_trash(&self, node: TreeID) -> Result<bool, ModelError> {
        let mut cursor = Some(node);
        while let Some(current) = cursor {
            if self.is_trashed(current)? {
                return Ok(true);
            }
            cursor = match self.tree.parent(current) {
                Some(TreeParentId::Node(up)) => Some(up),
                _ => None,
            };
        }
        Ok(false)
    }

    /// The page at `node` and every page below it, parents before children.
    fn collect_subtree(&self, node: TreeID, out: &mut Vec<PageId>) {
        out.extend(self.index.by_node.get(&node).copied());
        for child in self.tree.children(node).unwrap_or_default() {
            self.collect_subtree(child, out);
        }
    }
}

#[cfg(test)]
mod tests;
