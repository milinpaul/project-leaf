//! Reshaping a table from its controls (E16-S03): the "+" buttons and the row and column menus.
//!
//! Each edit is one table command and one undo step. The caret goes where the person is looking: into
//! the new cell after an insert, along with its cell after a move, and to the same place in the grid
//! (clamped) when its row or column was deleted.

use leaf_core::BlockId;
use leaf_model::{BlockKind, BlockProp, Caret, PageCommand};

use crate::table::cell_place;
use crate::{EditorError, PageEditor};

/// A change to a table's shape, from its controls. Positions count rows or columns from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableEdit {
    /// Add an empty row so it ends up at row `index` (the row count adds one at the bottom).
    InsertRow(usize),
    /// Delete row `index`. The last row can't go (delete the table instead).
    DeleteRow(usize),
    /// Move row `from` so it ends up at row `to`.
    MoveRow {
        /// Where it is.
        from: usize,
        /// Where it goes.
        to: usize,
    },
    /// Add an empty column so it ends up at column `index` (the column count adds one at the right).
    InsertColumn(usize),
    /// Delete column `index`. The last column can't go.
    DeleteColumn(usize),
    /// Move column `from` so it ends up at column `to`.
    MoveColumn {
        /// Where it is.
        from: usize,
        /// Where it goes.
        to: usize,
    },
    /// Turn the header row on or off.
    ToggleHeaderRow,
}

impl PageEditor {
    /// Applies `edit` to `table`, as one undo step, and returns where the caret is afterwards. An edit
    /// the table can't take (deleting its last row, a position past the end) is an error and changes
    /// nothing.
    pub fn edit_table(&mut self, table: BlockId, edit: TableEdit) -> Result<Option<Caret>, EditorError> {
        let BlockKind::Table { header_row } = self.doc.kind(table)? else {
            return Err(leaf_model::ModelError::NotATable(table).into());
        };
        // Where the caret sits in the grid now, if it is in this table.
        let place = match self.caret()? {
            Some(caret) => cell_place(&self.doc, caret.block)?
                .filter(|place| place.table == table)
                .map(|place| (place.row, place.column, caret.at)),
            None => None,
        };
        self.begin()?;
        let command = match edit {
            TableEdit::InsertRow(index) => PageCommand::InsertTableRow { table, index },
            TableEdit::DeleteRow(index) => PageCommand::DeleteTableRow { table, index },
            TableEdit::MoveRow { from, to } => PageCommand::MoveTableRow { table, from, to },
            TableEdit::InsertColumn(index) => PageCommand::InsertTableColumn { table, index },
            TableEdit::DeleteColumn(index) => PageCommand::DeleteTableColumn { table, index },
            TableEdit::MoveColumn { from, to } => PageCommand::MoveTableColumn { table, from, to },
            TableEdit::ToggleHeaderRow => PageCommand::SetProp {
                id: table,
                prop: BlockProp::TableHeaderRow(!header_row),
            },
        };
        self.doc.apply(command)?;
        let grid = self.doc.table_cells(table)?;
        let (row, column, at) = place.unwrap_or((0, 0, 0));
        let target = match edit {
            TableEdit::InsertRow(index) => Some((index, if place.is_some() { column } else { 0 }, 0)),
            TableEdit::InsertColumn(index) => Some((if place.is_some() { row } else { 0 }, index, 0)),
            TableEdit::MoveRow { from, to } if place.is_some() => Some((moved(row, from, to), column, at)),
            TableEdit::MoveColumn { from, to } if place.is_some() => Some((row, moved(column, from, to), at)),
            _ => place,
        };
        if let Some((row, column, at)) = target
            && let Some(cells) = grid.get(row.min(grid.len().saturating_sub(1)))
            && let Some(&block) = cells.get(column.min(cells.len().saturating_sub(1)))
        {
            let len = self.doc.plain_text(block)?.map_or(0, |t| t.chars().count());
            self.set_caret(Caret {
                block,
                at: at.min(len),
            })?;
        }
        self.finish()?;
        self.caret()
    }
}

/// Where position `at` ends up when the item at `from` moves to `to`.
fn moved(at: usize, from: usize, to: usize) -> usize {
    if at == from {
        to
    } else if from < at && at <= to {
        at.saturating_sub(1)
    } else if to <= at && at < from {
        at.saturating_add(1)
    } else {
        at
    }
}

#[cfg(test)]
mod tests;
