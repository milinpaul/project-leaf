//! The commands that change a database document. Like page and workspace commands, each is one Loro
//! commit and one undo step, and a refused command writes nothing.

use leaf_core::{ColumnId, OptionId, PageId};

use super::property::{CellValue, PropertyType};
use super::view::{Filter, Sort};

/// A change to a database.
#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseCommand {
    /// Adds a column at position `index` among the columns. Reports [`DatabaseOutcome::ColumnCreated`].
    AddColumn {
        /// Where it goes; equal to the column count appends.
        index: usize,
        /// Its name.
        name: String,
        /// Its type. Unknown types can't be created.
        kind: PropertyType,
    },
    /// Renames a column.
    RenameColumn {
        /// The column.
        column: ColumnId,
        /// The new name.
        name: String,
    },
    /// Changes a column's type. Stored cell values stay as they are (ADR-066 answer 3); options stay too, so
    /// changing back to a select brings them back. A column of an unknown type can't be retyped.
    SetColumnType {
        /// The column.
        column: ColumnId,
        /// The new type. Unknown types can't be set.
        kind: PropertyType,
    },
    /// Moves a column to position `index` among the columns (counted without it).
    MoveColumn {
        /// The column.
        column: ColumnId,
        /// Where it goes.
        index: usize,
    },
    /// Hides or shows a column in the table view.
    SetColumnHidden {
        /// The column.
        column: ColumnId,
        /// Whether it is hidden.
        hidden: bool,
    },
    /// Sets a column's width in the table view (`None` for the default).
    SetColumnWidth {
        /// The column.
        column: ColumnId,
        /// The width, in points; must be finite and positive.
        width: Option<f64>,
    },
    /// Deletes a column. Its cells stay in the document, unread, so undo brings it back whole (ADR-066).
    DeleteColumn {
        /// The column.
        column: ColumnId,
    },
    /// Adds an option at the end of a select or multi-select column's list. Reports
    /// [`DatabaseOutcome::OptionCreated`].
    AddOption {
        /// The column.
        column: ColumnId,
        /// Its name.
        name: String,
        /// Its colour name.
        color: String,
    },
    /// Renames an option. Cells hold its ID, so they show the new name at once.
    RenameOption {
        /// The column.
        column: ColumnId,
        /// The option.
        option: OptionId,
        /// The new name.
        name: String,
    },
    /// Changes an option's colour.
    SetOptionColor {
        /// The column.
        column: ColumnId,
        /// The option.
        option: OptionId,
        /// The colour name.
        color: String,
    },
    /// Deletes an option from the list. Cells that picked it keep its ID, which then reads as no option.
    DeleteOption {
        /// The column.
        column: ColumnId,
        /// The option.
        option: OptionId,
    },
    /// Adds a row at position `index`. The row is the page `row`, created by the caller (E09-S03).
    AddRow {
        /// The row's page.
        row: PageId,
        /// Where it goes; equal to the row count appends.
        index: usize,
    },
    /// Moves a row to position `index` among the rows (counted without it).
    MoveRow {
        /// The row.
        row: PageId,
        /// Where it goes.
        index: usize,
    },
    /// Takes a row out of the database. Its cells stay in the document, unread, so undo brings it back whole.
    DeleteRow {
        /// The row.
        row: PageId,
    },
    /// Brings a deleted row back where it was, cells and all (its page restored from the Trash, E09-S03).
    RestoreRow {
        /// The row.
        row: PageId,
    },
    /// Sets a cell, or empties it with `None`. The value must fit the column's type, and options must be the
    /// column's. Columns of an unknown type are read-only.
    SetCell {
        /// The row.
        row: PageId,
        /// The column.
        column: ColumnId,
        /// The value, or `None` to empty the cell.
        value: Option<CellValue>,
    },
    /// Replaces the view's sort.
    SetSort(Vec<Sort>),
    /// Replaces the view's filters.
    SetFilters(Vec<Filter>),
}

/// What a database command did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseOutcome {
    /// The command changed the database.
    Done,
    /// The command was valid but changed nothing (renaming to the same name, say).
    Unchanged,
    /// A column was added; here is its ID.
    ColumnCreated(ColumnId),
    /// An option was added; here is its ID.
    OptionCreated(OptionId),
}
