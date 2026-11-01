//! A database's own document (Phase 1, ADR-066): its columns, the order of its rows, every cell, and how the
//! table view sorts and filters.
//!
//! One `Database` owns one Loro document:
//!
//! ```text
//! "columns": LoroTree (flat)   column order; node meta "id" = ColumnId
//! "schema":  LoroMap           ColumnId -> map { name, type, hidden, width, options, ...unknown keys kept }
//! "rows":    LoroTree (flat)   row order; node meta "id" = the row's PageId
//! "cells":   LoroMap           PageId -> mergeable map: ColumnId -> value
//! "view":    LoroMap           sort, filters (each one plain value)
//! ```
//!
//! **Order and existence are kept apart.** The trees only order things: a delete and a move of the same node
//! on two devices settle by Loro's order of concurrent edits, so a deleted node can come back (E09-S02 found this; the E09-S01
//! spike had tried one order only). So a column exists while its schema entry does, and a row exists until
//! its node's meta says `deleted`; neither is ever taken out of its tree. A move never touches either record,
//! so a delete stays a delete. A node that a merge leaves twice reads once.
//!
//! Each row's cells are a *mergeable* map, so two devices filling in a row for the first time at once both
//! keep their cells. Cells of deleted rows and columns stay in `"cells"`, unread, so undo brings them back
//! whole and an edit that arrives after a delete isn't lost. Rows are pages: their titles and bodies live in the workspace and their page documents
//! (E09-S03); this document only knows their IDs.
//!
//! Like pages, the model talks about columns and rows by permanent IDs and keeps Loro's `TreeID`s private,
//! because undo can re-create nodes under new ones (ADR-006).

mod apply;
mod changes;
mod command;
mod index;
mod invariants;
mod options;
mod property;
mod view;

pub use changes::DatabaseChanges;
pub use command::{DatabaseCommand, DatabaseOutcome};
pub use property::{CellValue, Column, PropertyType, SelectOption};
pub use view::{Extra, Filter, FilterOp, Sort};

use leaf_core::{ColumnId, PageId};
use loro::{
    Container, ExportMode, LoroDoc, LoroMap, LoroTree, LoroValue, TreeID, UndoManager, ValueOrContainer,
};

use crate::page_meta::value;
use crate::saved::{self, SavedVersion};
use crate::{DEFAULT_UNDO_MERGE_MS, ModelError};
use index::{Index, live};

/// Names of the document's top-level containers.
mod containers {
    pub const COLUMNS: &str = "columns";
    pub const SCHEMA: &str = "schema";
    pub const ROWS: &str = "rows";
    pub const CELLS: &str = "cells";
    pub const VIEW: &str = "view";
}

/// Key of a column or row node's permanent ID in its tree meta.
const ID: &str = "id";

/// Key in a row node's meta set to `true` once the row is deleted. Rows stay in their tree (see the module docs).
const DELETED: &str = "deleted";

/// One database. Changed only through [`Database::apply`] and its [`DatabaseCommand`]s.
pub struct Database {
    doc: LoroDoc,
    columns: LoroTree,
    rows: LoroTree,
    index: Index,
    undo: UndoManager,
    saved: SavedVersion,
    pending: std::sync::Arc<std::sync::Mutex<changes::Pending>>,
    _events: loro::Subscription,
}

impl Database {
    /// An empty database: no columns, no rows.
    #[must_use]
    pub fn new() -> Self {
        let doc = LoroDoc::new();
        Self::over(doc, Index::default())
    }

    /// Loads a database from bytes produced by [`Database::to_bytes`] (or any Loro export of one). Fails if a
    /// column or row lacks a valid, unique ID, rather than hiding it.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        Self::from_saved(bytes, &[])
    }

    /// Loads a database the way the store keeps it: a snapshot plus the updates saved after it, in order.
    /// The loaded state counts as saved.
    pub fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError> {
        let doc = LoroDoc::new();
        saved::import(&doc, snapshot, updates)?;
        Self::from_doc(doc)
    }

    /// Loads what can be loaded after [`Database::from_saved`] failed: the snapshot plus every update that
    /// isn't damaged. Returns the positions in `updates` that were skipped.
    pub fn recover(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(Self, Vec<usize>), ModelError> {
        let doc = LoroDoc::new();
        let skipped = saved::import_each(&doc, snapshot, updates)?;
        Ok((Self::from_doc(doc)?, skipped))
    }

    fn from_doc(doc: LoroDoc) -> Result<Self, ModelError> {
        let index = Index::build(&columns_tree(&doc), &rows_tree(&doc))?;
        Ok(Self::over(doc, index))
    }

    /// Wraps a loaded document. Undo and the saved version start from its state as it is now.
    fn over(doc: LoroDoc, index: Index) -> Self {
        let columns = columns_tree(&doc);
        let rows = rows_tree(&doc);
        let mut undo = UndoManager::new(&doc);
        undo.set_merge_interval(DEFAULT_UNDO_MERGE_MS);
        let saved = SavedVersion::of(&doc);
        // Watching starts after the load, so loading itself isn't reported as a change.
        let pending = std::sync::Arc::default();
        let events = changes::watch(&doc, &pending);
        Self {
            doc,
            columns,
            rows,
            index,
            undo,
            saved,
            pending,
            _events: events,
        }
    }

    /// The whole database as a self-contained snapshot, ready to save.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Every change since the last [`Database::mark_saved`] (or load) as one update, or `None`.
    pub fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError> {
        self.saved.changes(&self.doc)
    }

    /// Records that the database as it is now is on disk. Call only after the write succeeded.
    pub fn mark_saved(&self) {
        self.saved.update(&self.doc);
    }

    /// Whether anything changed since the last save (or load).
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        self.saved.is_behind(&self.doc)
    }

    /// Merges changes from another copy of this database (another device, later: sync).
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        crate::loro_import::import(&self.doc, bytes)?;
        self.reindex()
    }

    /// Reverts the most recent undo step. Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<bool, ModelError> {
        let changed = self.undo.undo()?;
        if changed {
            self.reindex()?;
        }
        Ok(changed)
    }

    /// Re-applies the most recently undone step. Returns `false` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, ModelError> {
        let changed = self.undo.redo()?;
        if changed {
            self.reindex()?;
        }
        Ok(changed)
    }

    /// Runs `f` as one undo step: every command it applies undoes and redoes together. For an action made of
    /// several commands, such as creating an option and picking it in a cell.
    pub fn group<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, ModelError>) -> Result<T, ModelError> {
        self.undo.group_start()?;
        let result = f(self);
        self.undo.group_end();
        result
    }

    /// How close together, in milliseconds, commands must be to undo as one step. `0` makes every command its
    /// own step, which tests use.
    pub fn set_undo_merge_interval(&mut self, ms: i64) {
        self.undo.set_merge_interval(ms);
    }

    /// Undo, redo and merges can re-create nodes under new `TreeID`s, so the index is rebuilt from IDs.
    fn reindex(&mut self) -> Result<(), ModelError> {
        self.index = Index::build(&self.columns, &self.rows)?;
        Ok(())
    }

    /// The columns, in order, hidden ones included.
    #[must_use]
    pub fn column_ids(&self) -> Vec<ColumnId> {
        self.live_columns().into_iter().map(|(id, _)| id).collect()
    }

    /// Live columns in order with the node that places each: nodes of deleted columns (no schema) and repeats
    /// left by a merge are skipped.
    fn live_columns(&self) -> Vec<(ColumnId, TreeID)> {
        live(&self.columns, &self.index.columns, |column| {
            self.schema_of(column).is_some()
        })
    }

    /// Live rows in order with their nodes, like [`Database::live_columns`].
    fn live_rows(&self) -> Vec<(PageId, TreeID)> {
        live(&self.rows, &self.index.rows, |row| !self.row_deleted(row))
    }

    /// Whether `row`'s node is marked deleted (or missing).
    fn row_deleted(&self, row: PageId) -> bool {
        self.index.rows.by_id.get(&row).is_none_or(|&node| {
            self.rows
                .get_meta(node)
                .is_ok_and(|meta| matches!(value(&meta, DELETED), Some(LoroValue::Bool(true))))
        })
    }

    /// Every column, in order, hidden ones included.
    pub fn columns(&self) -> Result<Vec<Column>, ModelError> {
        self.column_ids().into_iter().map(|c| self.column(c)).collect()
    }

    /// One column's name, type, options and view settings.
    pub fn column(&self, column: ColumnId) -> Result<Column, ModelError> {
        let schema = self
            .schema_of(column)
            .filter(|_| self.index.columns.by_id.contains_key(&column))
            .ok_or(ModelError::ColumnNotFound(column))?;
        Ok(property::read_column(column, &schema))
    }

    /// The rows, in order.
    #[must_use]
    pub fn rows(&self) -> Vec<PageId> {
        self.live_rows().into_iter().map(|(id, _)| id).collect()
    }

    /// Whether `row` is a row of this database.
    #[must_use]
    pub fn contains_row(&self, row: PageId) -> bool {
        !self.row_deleted(row)
    }

    /// What a cell holds, or `None` when it is empty.
    pub fn cell(&self, row: PageId, column: ColumnId) -> Result<Option<CellValue>, ModelError> {
        if !self.contains_row(row) {
            return Err(ModelError::RowNotFound(row));
        }
        let kind = self.column(column)?.kind;
        Ok(self
            .row_cells(row)
            .and_then(|cells| cells.get(&column.to_string()))
            .and_then(|cell| CellValue::read(cell, &kind)))
    }

    /// Every value in `row`, by column, read in one go: what the table view caches per row. Includes values of
    /// deleted columns (the caller keeps only the columns it shows). Empty for a row with nothing set.
    pub fn row_values(&self, row: PageId) -> Result<Vec<(ColumnId, CellValue)>, ModelError> {
        if !self.contains_row(row) {
            return Err(ModelError::RowNotFound(row));
        }
        let Some(cells) = self.row_cells(row) else {
            return Ok(Vec::new());
        };
        let mut values = Vec::new();
        cells.for_each(|key, cell| {
            let Ok(column) = key.parse::<ColumnId>() else {
                return;
            };
            let kind = self.schema_of(column).map_or(PropertyType::Text, |schema| {
                property::read_column(column, &schema).kind
            });
            if let Some(value) = CellValue::read(cell, &kind) {
                values.push((column, value));
            }
        });
        Ok(values)
    }

    /// The view's sort, first key first.
    #[must_use]
    pub fn sort(&self) -> Vec<Sort> {
        view::read_sort(value(&self.view(), view::keys::SORT))
    }

    /// The view's filters.
    #[must_use]
    pub fn filters(&self) -> Vec<Filter> {
        view::read_filters(value(&self.view(), view::keys::FILTERS), |column| {
            self.column(column).ok().map(|c| c.kind)
        })
    }

    fn schema(&self) -> LoroMap {
        self.doc.get_map(containers::SCHEMA)
    }

    fn cells(&self) -> LoroMap {
        self.doc.get_map(containers::CELLS)
    }

    fn view(&self) -> LoroMap {
        self.doc.get_map(containers::VIEW)
    }

    fn schema_of(&self, column: ColumnId) -> Option<LoroMap> {
        match self.schema().get(&column.to_string()) {
            Some(ValueOrContainer::Container(Container::Map(map))) => Some(map),
            _ => None,
        }
    }

    /// A row's cells map, if anything was ever set in the row.
    fn row_cells(&self, row: PageId) -> Option<LoroMap> {
        match self.cells().get(&row.to_string()) {
            Some(ValueOrContainer::Container(Container::Map(map))) => Some(map),
            _ => None,
        }
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}

fn columns_tree(doc: &LoroDoc) -> LoroTree {
    let tree = doc.get_tree(containers::COLUMNS);
    tree.enable_fractional_index(0);
    tree
}

fn rows_tree(doc: &LoroDoc) -> LoroTree {
    let tree = doc.get_tree(containers::ROWS);
    tree.enable_fractional_index(0);
    tree
}

#[cfg(test)]
mod tests;
