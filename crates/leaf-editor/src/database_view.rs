//! A database as a table (E09-S04): the columns to show, the rows in order, and what each cell shows.
//!
//! [`DatabaseEditor`] owns a [`Database`] and keeps the cells it has read, by row. Reading every cell of a big
//! database takes longer than opening a page may (ADR 0066 measured about 190 ms for 10,000 rows), so the cells are
//! read a chunk of rows at a time, in the database's own order: [`DatabaseEditor::read_more`] reads the next chunk,
//! and the UI calls it on idle ticks until [`DatabaseEditor::is_complete`]. Nothing here uses threads, so the crate
//! stays wasm-ok. The first screen is ready after one chunk; with a sort or filter on, the view says it is not
//! complete until every row is read.
//!
//! Every change (a command, undo, redo, or later a merge from another device) is picked up from the database's own
//! change record ([`Database::take_changes`]): only rows whose cells changed are read again, and rows that appear are
//! read at once. Columns and the view are read fresh each time, since they are small.

mod display;
mod order;

pub use display::{CellDisplay, Chip, date_label, display};

use std::collections::{HashMap, HashSet};

use leaf_core::{ColumnId, PageId};
use leaf_model::{Database, DatabaseCommand, DatabaseOutcome, ModelError, PropertyType};

use order::Values;

/// Width of a column that was never resized, in points.
pub const DEFAULT_COLUMN_WIDTH: f64 = 180.0;

/// How many rows a first screen needs; the UI asks for this many before drawing a database.
pub const FIRST_SCREEN_ROWS: usize = 50;

/// One column of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewColumn {
    /// The column.
    pub id: ColumnId,
    /// Its name.
    pub name: String,
    /// Its type, for the header icon and the cell editor.
    pub kind: PropertyType,
    /// Its width in points.
    pub width: f64,
}

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRow {
    /// The row's page.
    pub row: PageId,
    /// The row page's title (the first column), or "Untitled".
    pub title: String,
    /// What each shown column's cell shows, in column order.
    pub cells: Vec<CellDisplay>,
}

/// A database ready to draw as a table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableView {
    /// The shown columns, in order (hidden ones left out).
    pub columns: Vec<ViewColumn>,
    /// The rows read so far that pass the filters, in the view's order.
    pub rows: Vec<ViewRow>,
    /// Whether every row has been read. When not, rows may still be missing (and, with a sort, out of place).
    pub complete: bool,
    /// How many rows the database has, filtered or not.
    pub total_rows: usize,
}

/// A database being shown as a table: the database, and the cells read from it so far.
pub struct DatabaseEditor {
    db: Database,
    /// The rows in the database's order.
    order: Vec<PageId>,
    /// The rows read so far, with their values.
    cells: HashMap<PageId, Values>,
    /// How many times a row's cells were read, for tests and measurements.
    reads: usize,
}

impl DatabaseEditor {
    /// Starts showing `db`, having read none of its cells yet.
    #[must_use]
    pub fn new(db: Database) -> Self {
        db.take_changes();
        let order = db.rows();
        Self {
            db,
            order,
            cells: HashMap::new(),
            reads: 0,
        }
    }

    /// The database, for reading.
    #[must_use]
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Applies a command to the database. The view picks the change up when next built.
    pub fn apply(&mut self, command: DatabaseCommand) -> Result<DatabaseOutcome, ModelError> {
        self.db.apply(command)
    }

    /// Undoes the database's last step. Returns `false` when there was nothing to undo.
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        self.db.undo()
    }

    /// Redoes the database's last undone step. Returns `false` when there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        self.db.redo()
    }

    /// How close together, in milliseconds, commands must be to undo as one step (`0`: each its own).
    pub fn set_undo_merge_interval(&mut self, ms: i64) {
        self.db.set_undo_merge_interval(ms);
    }

    /// Reads the cells of up to `rows` more rows, in the database's order, and returns whether every row is now
    /// read.
    pub fn read_more(&mut self, rows: usize) -> Result<bool, ModelError> {
        self.refresh()?;
        let next: Vec<PageId> = self
            .order
            .iter()
            .filter(|row| !self.cells.contains_key(row))
            .take(rows)
            .copied()
            .collect();
        for row in next {
            self.read_row(row)?;
        }
        self.is_complete()
    }

    /// Whether every row's cells have been read (after picking up any change).
    pub fn is_complete(&mut self) -> Result<bool, ModelError> {
        self.refresh()?;
        Ok(self.order.iter().all(|row| self.cells.contains_key(row)))
    }

    /// How many times a row's cells have been read. The first screen reads only its rows, and an edit reads only
    /// the rows it touched.
    #[must_use]
    pub fn rows_read(&self) -> usize {
        self.reads
    }

    fn read_row(&mut self, row: PageId) -> Result<(), ModelError> {
        let values = self.db.row_values(row)?.into_iter().collect();
        self.cells.insert(row, values);
        self.reads += 1;
        Ok(())
    }

    /// Brings the cache in line with what changed in the database since last time.
    fn refresh(&mut self) -> Result<(), ModelError> {
        let changes = self.db.take_changes();
        if changes.everything {
            self.order = self.db.rows();
            self.cells.clear();
            return Ok(());
        }
        if changes.order {
            let before: HashSet<PageId> = self.order.iter().copied().collect();
            self.order = self.db.rows();
            let live: HashSet<PageId> = self.order.iter().copied().collect();
            self.cells.retain(|row, _| live.contains(row));
            // Rows that just appeared (added, restored, merged in) are read now, so they show at once.
            let appeared: Vec<PageId> = self
                .order
                .iter()
                .filter(|r| !before.contains(r))
                .copied()
                .collect();
            for row in appeared {
                self.read_row(row)?;
            }
        }
        let stale: Vec<PageId> = changes
            .rows
            .into_iter()
            .filter(|row| self.cells.contains_key(row))
            .collect();
        for row in stale {
            self.read_row(row)?;
        }
        Ok(())
    }

    /// The table as it stands: shown columns, and the rows read so far, filtered and sorted. `title` gives a row
    /// page's title; `offset_minutes` is the device's time zone, for dates.
    pub fn view(
        &mut self,
        title: impl Fn(PageId) -> String,
        offset_minutes: i32,
    ) -> Result<TableView, ModelError> {
        self.refresh()?;
        let columns = self.db.columns()?;
        let (sorts, filters) = (self.db.sort(), self.db.filters());
        let complete = self.order.iter().all(|row| self.cells.contains_key(row));
        let empty = Values::new();
        let mut rows: Vec<PageId> = self
            .order
            .iter()
            .filter(|row| {
                self.cells
                    .get(row)
                    .is_some_and(|values| order::passes(values, &filters, &columns))
            })
            .copied()
            .collect();
        order::sort(&mut rows, &sorts, &columns, &self.cells);
        let shown: Vec<_> = columns.iter().filter(|c| !c.hidden).collect();
        Ok(TableView {
            columns: shown
                .iter()
                .map(|c| ViewColumn {
                    id: c.id,
                    name: c.name.clone(),
                    kind: c.kind.clone(),
                    width: c.width.unwrap_or(DEFAULT_COLUMN_WIDTH),
                })
                .collect(),
            rows: rows
                .into_iter()
                .map(|row| {
                    let values = self.cells.get(&row).unwrap_or(&empty);
                    let name = title(row);
                    ViewRow {
                        row,
                        title: if name.is_empty() {
                            crate::UNTITLED.to_owned()
                        } else {
                            name
                        },
                        cells: shown
                            .iter()
                            .map(|c| display(values.get(&c.id), c, offset_minutes))
                            .collect(),
                    }
                })
                .collect(),
            complete,
            total_rows: self.order.len(),
        })
    }
}

#[cfg(test)]
mod tests;
