//! Tables (E16, ADR 0061): building a grid of cells and reshaping it by rows and columns.
//!
//! A table block holds row blocks, which hold cell blocks; a cell is a text block like any other, so
//! its text and marks change through the usual text commands. Columns are positions within rows.
//! Every command here keeps a table rectangular with at least one row and one column. Merging two
//! copies can leave rows of different lengths (one copy added a column while the other added a row);
//! that is allowed, a missing cell reads as empty, and the next table command evens the rows out
//! as part of the user's change. Evening them out in the merge repair instead would create cells
//! on both copies, with different IDs, so the copies would no longer match.

use leaf_core::BlockId;
use loro::TreeID;

use super::PageDoc;
use crate::block_kind::keys;
use crate::{BlockKind, ModelError, PageOutcome};

impl PageDoc {
    /// The cells of `table`, row by row, each row in column order. Rows are all the same length after
    /// any table command; after merging another copy's changes they may differ until the next one.
    pub fn table_cells(&self, table: BlockId) -> Result<Vec<Vec<BlockId>>, ModelError> {
        let node = self.table_node(table)?;
        Ok(self
            .child_nodes(Some(node))
            .into_iter()
            .map(|row| self.index.blocks_for(self.child_nodes(Some(row))))
            .collect())
    }

    pub(super) fn insert_table(
        &mut self,
        parent: Option<BlockId>,
        index: usize,
        rows: usize,
        columns: usize,
        header_row: bool,
    ) -> Result<PageOutcome, ModelError> {
        if rows == 0 || columns == 0 {
            return Err(ModelError::EmptyTable);
        }
        let parent_node = self.container_node(parent)?;
        let len = self.tree.children_num(parent_node).unwrap_or(0);
        if index > len {
            return Err(ModelError::IndexOutOfRange { index, len });
        }
        let (table, _) = self.create_block(parent_node, index, &BlockKind::Table { header_row })?;
        let node = self.node(table)?;
        for row in 0..rows {
            self.add_row(node, row, columns)?;
        }
        Ok(PageOutcome::BlockCreated(table))
    }

    pub(super) fn set_header_row(
        &mut self,
        table: BlockId,
        header_row: bool,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        if self.kind(table)? != (BlockKind::Table { header_row }) {
            self.tree.get_meta(node)?.insert(keys::HEADER_ROW, header_row)?;
        }
        Ok(PageOutcome::Changed)
    }

    pub(super) fn insert_table_row(
        &mut self,
        table: BlockId,
        index: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        let columns = self.even_out(node)?;
        let rows = self.child_nodes(Some(node)).len();
        if index > rows {
            return Err(ModelError::IndexOutOfRange { index, len: rows });
        }
        let row = self.add_row(node, index, columns)?;
        Ok(PageOutcome::BlockCreated(row))
    }

    pub(super) fn delete_table_row(
        &mut self,
        table: BlockId,
        index: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        let rows = self.child_nodes(Some(node));
        let row = *rows.get(index).ok_or(ModelError::IndexOutOfRange {
            index,
            len: rows.len(),
        })?;
        if rows.len() == 1 {
            return Err(ModelError::EmptyTable);
        }
        self.even_out(node)?;
        Ok(PageOutcome::BlocksDeleted(self.delete_nodes(&[row])?))
    }

    pub(super) fn move_table_row(
        &mut self,
        table: BlockId,
        from: usize,
        to: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        let rows = self.child_nodes(Some(node));
        check_positions(from, to, rows.len())?;
        self.even_out(node)?;
        self.move_within(&rows, from, to)?;
        Ok(PageOutcome::Changed)
    }

    pub(super) fn insert_table_column(
        &mut self,
        table: BlockId,
        index: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        let columns = self.table_width(node);
        if index > columns {
            return Err(ModelError::IndexOutOfRange { index, len: columns });
        }
        self.even_out(node)?;
        let mut cells = Vec::new();
        for row in self.child_nodes(Some(node)) {
            let (cell, _) = self.create_block(Some(row), index, &BlockKind::TableCell)?;
            cells.push(cell);
        }
        Ok(PageOutcome::BlocksCreated(cells))
    }

    pub(super) fn delete_table_column(
        &mut self,
        table: BlockId,
        index: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        let columns = self.table_width(node);
        if index >= columns {
            return Err(ModelError::IndexOutOfRange { index, len: columns });
        }
        if columns == 1 {
            return Err(ModelError::EmptyTable);
        }
        self.even_out(node)?;
        let cells: Vec<TreeID> = self
            .child_nodes(Some(node))
            .into_iter()
            .filter_map(|row| self.child_nodes(Some(row)).get(index).copied())
            .collect();
        Ok(PageOutcome::BlocksDeleted(self.delete_nodes(&cells)?))
    }

    pub(super) fn move_table_column(
        &mut self,
        table: BlockId,
        from: usize,
        to: usize,
    ) -> Result<PageOutcome, ModelError> {
        let node = self.table_node(table)?;
        check_positions(from, to, self.table_width(node))?;
        self.even_out(node)?;
        for row in self.child_nodes(Some(node)) {
            let cells = self.child_nodes(Some(row));
            self.move_within(&cells, from, to)?;
        }
        Ok(PageOutcome::Changed)
    }

    /// The table node for `table`, or an error if it isn't a table.
    fn table_node(&self, table: BlockId) -> Result<TreeID, ModelError> {
        match self.kind(table)? {
            BlockKind::Table { .. } => self.node(table),
            _ => Err(ModelError::NotATable(table)),
        }
    }

    /// Adds a row of `columns` empty cells at row position `index`.
    fn add_row(&mut self, table: TreeID, index: usize, columns: usize) -> Result<BlockId, ModelError> {
        let (row, _) = self.create_block(Some(table), index, &BlockKind::TableRow)?;
        let row_node = self.node(row)?;
        for column in 0..columns {
            self.create_block(Some(row_node), column, &BlockKind::TableCell)?;
        }
        Ok(row)
    }

    /// The number of columns: the longest row (at least 1, so even a merged-away table has a column).
    fn table_width(&self, table: TreeID) -> usize {
        self.child_nodes(Some(table))
            .into_iter()
            .map(|row| self.child_nodes(Some(row)).len())
            .max()
            .unwrap_or(0)
            .max(1)
    }

    /// Gives every row the same number of cells (and an emptied table a row), adding empty cells at
    /// the end of short rows, and returns the number of columns. Only needed after a merge left rows
    /// of different lengths; runs inside the table command, as part of the user's change.
    fn even_out(&mut self, table: TreeID) -> Result<usize, ModelError> {
        let columns = self.table_width(table);
        if self.child_nodes(Some(table)).is_empty() {
            self.add_row(table, 0, columns)?;
        }
        for row in self.child_nodes(Some(table)) {
            for column in self.child_nodes(Some(row)).len()..columns {
                self.create_block(Some(row), column, &BlockKind::TableCell)?;
            }
        }
        Ok(columns)
    }

    /// Moves `nodes[from]` so it ends up at position `to` among `nodes` (all siblings).
    fn move_within(&self, nodes: &[TreeID], from: usize, to: usize) -> Result<(), ModelError> {
        if from == to {
            return Ok(());
        }
        let moving = nodes[from];
        let others: Vec<TreeID> = nodes.iter().copied().filter(|&n| n != moving).collect();
        match others.get(to) {
            Some(&next) => self.tree.mov_before(moving, next)?,
            None => self.tree.mov_after(moving, others[others.len() - 1])?,
        }
        Ok(())
    }

    /// Deletes `nodes` with everything inside them; returns the deleted blocks, parents first.
    fn delete_nodes(&mut self, nodes: &[TreeID]) -> Result<Vec<BlockId>, ModelError> {
        let mut deleted = Vec::new();
        for &node in nodes {
            self.collect_subtree(node, &mut deleted);
            self.tree.delete(node)?;
        }
        for &block in &deleted {
            self.index.remove(block);
        }
        Ok(deleted)
    }

    fn child_nodes(&self, parent: Option<TreeID>) -> Vec<TreeID> {
        self.tree.children(parent).unwrap_or_default()
    }

    /// Fails if any of `nodes` is a row or cell: the general block commands leave those alone.
    pub(super) fn refuse_table_parts(&self, nodes: &[TreeID]) -> Result<(), ModelError> {
        for &node in nodes {
            let Some(&block) = self.index.by_node.get(&node) else {
                continue;
            };
            if self.kind(block)?.is_table_part() {
                return Err(ModelError::TablePart { block });
            }
        }
        Ok(())
    }

    /// The table a row or cell belongs to.
    pub(super) fn table_of(&self, part: BlockId) -> Result<BlockId, ModelError> {
        let mut block = part;
        while let Some(parent) = self.parent(block)? {
            if matches!(self.kind(parent)?, BlockKind::Table { .. }) {
                return Ok(parent);
            }
            block = parent;
        }
        Err(ModelError::TablePart { block: part })
    }
}

/// Checks a move between two positions of a list of `len`.
fn check_positions(from: usize, to: usize, len: usize) -> Result<(), ModelError> {
    for index in [from, to] {
        if index >= len {
            return Err(ModelError::IndexOutOfRange { index, len });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
