//! Tab and Shift+Tab: indenting a block under the one above it, and outdenting it to its parent's level.
//!
//! Selected blocks are handled one at a time in reading order, which gives the familiar result for a
//! run of consecutive blocks: they move together and keep their order. A block that can't move is
//! skipped; if none can, nothing is written and the command reports `Unchanged`.

use leaf_core::BlockId;
use loro::TreeID;

use super::PageDoc;
use crate::{ModelError, PageOutcome};

impl PageDoc {
    pub(super) fn indent(&mut self, ids: &[BlockId]) -> Result<PageOutcome, ModelError> {
        let roots = self.outermost(ids)?;
        self.refuse_table_parts(&roots)?;
        for node in roots {
            let siblings = self.tree.children(self.parent_node(node)).unwrap_or_default();
            let position = siblings.iter().position(|&n| n == node).unwrap_or(0);
            let Some(&above) = position.checked_sub(1).and_then(|p| siblings.get(p)) else {
                continue;
            };
            if !self.node_kind(above)?.allows_children() {
                continue;
            }
            let end = self.tree.children_num(above).unwrap_or(0);
            self.tree.mov_to(node, above, end)?;
        }
        Ok(PageOutcome::Changed)
    }

    pub(super) fn outdent(&mut self, ids: &[BlockId]) -> Result<PageOutcome, ModelError> {
        let roots = self.outermost(ids)?;
        self.refuse_table_parts(&roots)?;
        for node in roots {
            let Some(parent) = self.parent_node(node) else {
                continue;
            };
            let siblings = self.tree.children(parent).unwrap_or_default();
            let position = siblings.iter().position(|&n| n == node).unwrap_or(0);
            let followers = siblings[position + 1..].to_vec();
            if !followers.is_empty() && !self.node_kind(node)?.allows_children() {
                continue;
            }
            self.tree.mov_after(node, parent)?;
            for follower in followers {
                let end = self.tree.children_num(node).unwrap_or(0);
                self.tree.mov_to(follower, node, end)?;
            }
        }
        Ok(PageOutcome::Changed)
    }

    fn node_kind(&self, node: TreeID) -> Result<crate::BlockKind, ModelError> {
        let block = self
            .index
            .by_node
            .get(&node)
            .copied()
            .ok_or_else(|| ModelError::InvalidBlock {
                node: format!("{node:?}"),
                reason: "not indexed".into(),
            })?;
        self.kind(block)
    }
}

#[cfg(test)]
mod tests;
