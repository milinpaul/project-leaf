//! A row's properties on its own page (E09-S08): every column of the database, in column order, with the row's
//! value as the table shows it. Hidden columns are listed too, since the page is where a row is read in full.
//! Editing a property is the table's per-cell editing (`edit.rs`) with the same row and column, so a change on the
//! page is the same command, the same undo step and the same value the table shows.

use leaf_core::{ColumnId, PageId};
use leaf_model::{ModelError, PropertyType};

use super::display::display;
use super::{CellDisplay, DatabaseEditor};

/// One property of a row, as its page lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct RowProperty {
    /// The column.
    pub column: ColumnId,
    /// The column's name.
    pub name: String,
    /// The column's type, for its icon and editor.
    pub kind: PropertyType,
    /// What the row holds there, as the table shows it.
    pub display: CellDisplay,
}

impl DatabaseEditor {
    /// The properties of `row`, in column order. `offset_minutes` is the device's time zone, for dates. A row the
    /// database doesn't have (deleted, or from another database) has none.
    pub fn row_properties(&self, row: PageId, offset_minutes: i32) -> Result<Vec<RowProperty>, ModelError> {
        if !self.db.contains_row(row) {
            return Ok(Vec::new());
        }
        self.db
            .columns()?
            .into_iter()
            .map(|column| {
                let value = self.db.cell(row, column.id)?;
                Ok(RowProperty {
                    display: display(value.as_ref(), &column, offset_minutes),
                    column: column.id,
                    name: column.name,
                    kind: column.kind,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
