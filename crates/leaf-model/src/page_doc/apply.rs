//! Applying page commands: check everything that could fail, write, then commit once.

use std::collections::HashSet;

use leaf_core::BlockId;
use loro::{LoroText, TreeID, TreeParentId};

use super::PageDoc;
use crate::block_kind::keys;
use crate::{BlockKind, ModelError, PageCommand, PageOutcome};

impl PageDoc {
    /// Applies one command as exactly one Loro commit. On error, nothing is written. A valid command
    /// that changes nothing reports [`PageOutcome::Unchanged`].
    pub fn apply(&mut self, command: PageCommand) -> Result<PageOutcome, ModelError> {
        let before = self.doc.oplog_vv();
        let outcome = match command {
            PageCommand::InsertBlock {
                parent,
                index,
                kind,
                text,
            } => self.insert_block(parent, index, &kind, &text)?,
            PageCommand::DeleteBlocks { ids } => self.delete_blocks(&ids)?,
            PageCommand::MoveBlocks {
                ids,
                new_parent,
                index,
            } => self.move_blocks(&ids, new_parent, index)?,
            PageCommand::InsertText { id, pos, text } => self.insert_text(id, pos, &text)?,
            PageCommand::DeleteText { id, range } => self.delete_text(id, range)?,
            PageCommand::ReplaceText { id, range, text } => self.replace_text(id, range, &text)?,
            PageCommand::InsertPageLink { id, pos, page } => self.insert_page_link(id, pos, page)?,
            PageCommand::SetMark { id, range, mark } => self.set_mark(id, range, &mark)?,
            PageCommand::RemoveMark { id, range, kind } => self.remove_mark(id, range, kind)?,
            PageCommand::SetBlockKind { id, kind } => self.set_kind(id, &kind)?,
            PageCommand::SetProp { id, prop } => match self.kind(id)?.with_prop(prop)? {
                BlockKind::Table { header_row } => self.set_header_row(id, header_row)?,
                kind => self.set_kind(id, &kind)?,
            },
            PageCommand::SplitBlock { id, pos } => self.split_block(id, pos)?,
            PageCommand::MergeIntoPrevious { id } => self.merge_into_previous(id)?,
            PageCommand::DuplicateBlocks { ids } => self.duplicate_blocks(&ids)?,
            PageCommand::Indent { ids } => self.indent(&ids)?,
            PageCommand::Outdent { ids } => self.outdent(&ids)?,
            PageCommand::InsertTable {
                parent,
                index,
                rows,
                columns,
                header_row,
            } => self.insert_table(parent, index, rows, columns, header_row)?,
            PageCommand::InsertTableRow { table, index } => self.insert_table_row(table, index)?,
            PageCommand::DeleteTableRow { table, index } => self.delete_table_row(table, index)?,
            PageCommand::MoveTableRow { table, from, to } => self.move_table_row(table, from, to)?,
            PageCommand::InsertTableColumn { table, index } => self.insert_table_column(table, index)?,
            PageCommand::DeleteTableColumn { table, index } => self.delete_table_column(table, index)?,
            PageCommand::MoveTableColumn { table, from, to } => self.move_table_column(table, from, to)?,
        };
        self.doc.commit();
        if self.doc.oplog_vv() != before {
            self.after_commit();
        }
        if outcome == PageOutcome::Changed && self.doc.oplog_vv() == before {
            return Ok(PageOutcome::Unchanged);
        }
        Ok(outcome)
    }

    fn insert_block(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        kind: &BlockKind,
        text: &str,
    ) -> Result<PageOutcome, ModelError> {
        kind.check_creatable()?;
        super::page_links::check_plain(text)?;
        if !text.is_empty() && !kind.has_text() {
            return Err(ModelError::TextNotAllowed {
                kind: kind.type_name().to_owned(),
            });
        }
        let parent_node = self.container_node(parent)?;
        let len = self.tree.children_num(parent_node).unwrap_or(0);
        if index > len {
            return Err(ModelError::IndexOutOfRange { index, len });
        }

        let (block, container) = self.create_block(parent_node, index, kind)?;
        if let Some(container) = container.filter(|_| !text.is_empty()) {
            container.insert(0, text)?;
        }
        Ok(PageOutcome::BlockCreated(block))
    }

    /// Creates a block node with a fresh ID, its kind, and (for kinds with text) an empty text
    /// container, which it returns. Callers have already checked the position and kind.
    pub(super) fn create_block(
        &mut self,
        parent_node: Option<TreeID>,
        index: usize,
        kind: &BlockKind,
    ) -> Result<(BlockId, Option<LoroText>), ModelError> {
        let block = BlockId::new();
        let node = self.tree.create_at(parent_node, index)?;
        let meta = self.tree.get_meta(node)?;
        meta.insert(keys::ID, block.to_string())?;
        kind.write(&meta)?;
        // Mergeable: two peers creating it concurrently get one shared container (ADR 0001).
        let text = if kind.has_text() {
            Some(meta.ensure_mergeable_text(keys::TEXT)?)
        } else {
            None
        };
        self.index.insert(block, node);
        Ok((block, text))
    }

    fn delete_blocks(&mut self, ids: &[BlockId]) -> Result<PageOutcome, ModelError> {
        let roots = self.outermost(ids)?;
        self.refuse_table_parts(&roots)?;
        if roots.is_empty() {
            return Ok(PageOutcome::Unchanged);
        }
        let mut deleted = Vec::new();
        for &root in &roots {
            self.collect_subtree(root, &mut deleted);
        }
        for &root in &roots {
            self.tree.delete(root)?;
        }
        for &block in &deleted {
            self.index.remove(block);
        }
        Ok(PageOutcome::BlocksDeleted(deleted))
    }

    fn move_blocks(
        &mut self,
        ids: &[BlockId],
        new_parent: Option<BlockId>,
        index: usize,
    ) -> Result<PageOutcome, ModelError> {
        let roots = self.outermost(ids)?;
        self.refuse_table_parts(&roots)?;
        let parent_node = self.container_node(new_parent)?;
        if roots.is_empty() {
            return Ok(PageOutcome::Unchanged);
        }
        // Walking up from the destination must never meet a block being moved.
        let moving: HashSet<TreeID> = roots.iter().copied().collect();
        let mut cursor = parent_node;
        while let Some(current) = cursor {
            if moving.contains(&current) {
                let block = self.index.by_node[&current];
                let new_parent = new_parent.unwrap_or(block);
                return Err(ModelError::BlockMoveIntoOwnSubtree { block, new_parent });
            }
            cursor = self.parent_node(current);
        }
        let others: Vec<TreeID> = self
            .tree
            .children(parent_node)
            .unwrap_or_default()
            .into_iter()
            .filter(|n| !moving.contains(n))
            .collect();
        if index > others.len() {
            return Err(ModelError::IndexOutOfRange {
                index,
                len: others.len(),
            });
        }

        // First block goes before the sibling now at `index` (or after the last one); each later block
        // goes right after the one before it, so the set stays together and in document order.
        let mut previous: Option<TreeID> = None;
        for &node in &roots {
            match (previous, others.get(index), others.last()) {
                (Some(prev), _, _) => self.tree.mov_after(node, prev)?,
                (None, Some(&next), _) => self.tree.mov_before(node, next)?,
                (None, None, Some(&last)) => self.tree.mov_after(node, last)?,
                (None, None, None) => self.tree.mov_to(node, parent_node, 0)?,
            }
            previous = Some(node);
        }
        Ok(PageOutcome::Changed)
    }

    /// The node blocks would be placed in: `None` for the top level, or a block whose kind allows
    /// children.
    pub(super) fn container_node(&self, block: Option<BlockId>) -> Result<Option<TreeID>, ModelError> {
        let Some(block) = block else { return Ok(None) };
        let node = self.node(block)?;
        let kind = self.kind(block)?;
        if !kind.allows_children() {
            return Err(ModelError::ChildrenNotAllowed {
                parent: block,
                kind: kind.type_name().to_owned(),
            });
        }
        Ok(Some(node))
    }

    /// The listed blocks that are not inside another listed block, in document order. Fails if any
    /// block is unknown.
    pub(super) fn outermost(&self, ids: &[BlockId]) -> Result<Vec<TreeID>, ModelError> {
        let listed: HashSet<TreeID> = ids.iter().map(|&b| self.node(b)).collect::<Result<_, _>>()?;
        let has_listed_ancestor = |node: TreeID| {
            let mut cursor = self.parent_node(node);
            while let Some(current) = cursor {
                if listed.contains(&current) {
                    return true;
                }
                cursor = self.parent_node(current);
            }
            false
        };
        Ok(self
            .document_order()
            .into_iter()
            .filter(|n| listed.contains(n) && !has_listed_ancestor(*n))
            .collect())
    }

    /// Every block node, parents before children, in reading order.
    pub(super) fn document_order(&self) -> Vec<TreeID> {
        fn walk(doc: &PageDoc, nodes: Vec<TreeID>, out: &mut Vec<TreeID>) {
            for node in nodes {
                out.push(node);
                walk(doc, doc.tree.children(node).unwrap_or_default(), out);
            }
        }
        let mut out = Vec::with_capacity(self.index.by_block.len());
        walk(self, self.tree.roots(), &mut out);
        out
    }

    /// The block at `node` and every block inside it, parents first.
    pub(super) fn collect_subtree(&self, node: TreeID, out: &mut Vec<BlockId>) {
        out.extend(self.index.by_node.get(&node).copied());
        for child in self.tree.children(node).unwrap_or_default() {
            self.collect_subtree(child, out);
        }
    }

    pub(super) fn parent_node(&self, node: TreeID) -> Option<TreeID> {
        match self.tree.parent(node) {
            Some(TreeParentId::Node(parent)) => Some(parent),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
