//! Applying database commands: check everything that could fail, write, then commit once.

use leaf_core::{ColumnId, PageId};
use loro::{
    Container, LoroMap, LoroMovableList, LoroTree, LoroValue, TreeID, TreeParentId, ValueOrContainer,
};

use super::property::{CellValue, PropertyType, check_options, keys, write_options};
use super::{Database, DatabaseCommand, DatabaseOutcome, ID, view};
use crate::ModelError;
use crate::page_meta::value;

impl Database {
    /// Applies one command as exactly one Loro commit (one undo step). On error, nothing is written. A valid
    /// command that changes nothing reports [`DatabaseOutcome::Unchanged`].
    pub fn apply(&mut self, command: DatabaseCommand) -> Result<DatabaseOutcome, ModelError> {
        let before = self.doc.oplog_vv();
        let outcome = match command {
            DatabaseCommand::AddColumn { index, name, kind } => self.add_column(index, &name, &kind)?,
            DatabaseCommand::RenameColumn { column, name } => {
                self.set_schema(column, keys::NAME, name.as_str().into())?
            }
            DatabaseCommand::SetColumnType { column, kind } => self.set_type(column, &kind)?,
            DatabaseCommand::MoveColumn { column, index } => {
                let node = self.column_node(column)?;
                let live: Vec<TreeID> = self.live_columns().into_iter().map(|(_, n)| n).collect();
                move_node(&self.columns, &live, node, index)?
            }
            DatabaseCommand::SetColumnHidden { column, hidden } => {
                self.set_schema(column, keys::HIDDEN, hidden.into())?
            }
            DatabaseCommand::SetColumnWidth { column, width } => self.set_width(column, width)?,
            DatabaseCommand::DeleteColumn { column } => self.delete_column(column)?,
            DatabaseCommand::AddOption { column, name, color } => self.add_option(column, &name, &color)?,
            DatabaseCommand::RenameOption { column, option, name } => {
                self.set_option(column, option, keys::NAME, &name)?
            }
            DatabaseCommand::SetOptionColor {
                column,
                option,
                color,
            } => self.set_option(column, option, keys::COLOR, &color)?,
            DatabaseCommand::DeleteOption { column, option } => self.delete_option(column, option)?,
            DatabaseCommand::MoveOption {
                column,
                option,
                index,
            } => self.move_option(column, option, index)?,
            DatabaseCommand::AddRow { row, index } => self.add_row(row, index)?,
            DatabaseCommand::MoveRow { row, index } => {
                let node = self.row_node(row)?;
                let live: Vec<TreeID> = self.live_rows().into_iter().map(|(_, n)| n).collect();
                move_node(&self.rows, &live, node, index)?
            }
            // Marked, not taken out of the tree: a move on another device must not bring it back.
            DatabaseCommand::DeleteRow { row } => {
                let node = self.row_node(row)?;
                self.rows.get_meta(node)?.insert(super::DELETED, true)?;
                DatabaseOutcome::Done
            }
            DatabaseCommand::RestoreRow { row } => self.restore_row(row)?,
            DatabaseCommand::SetCell { row, column, value } => self.set_cell(row, column, value.as_ref())?,
            DatabaseCommand::SetSort(sort) => self.set_view(
                view::keys::SORT,
                view::write_sort(&sort),
                &view::write_sort(&self.sort()),
            )?,
            DatabaseCommand::SetFilters(filters) => self.set_view(
                view::keys::FILTERS,
                view::write_filters(&filters),
                &view::write_filters(&self.filters()),
            )?,
        };
        self.doc.commit();
        if self.doc.oplog_vv() == before && outcome == DatabaseOutcome::Done {
            return Ok(DatabaseOutcome::Unchanged);
        }
        Ok(outcome)
    }

    fn column_node(&self, column: ColumnId) -> Result<TreeID, ModelError> {
        self.column(column)?;
        self.index
            .columns
            .by_id
            .get(&column)
            .copied()
            .ok_or(ModelError::ColumnNotFound(column))
    }

    fn row_node(&self, row: PageId) -> Result<TreeID, ModelError> {
        if !self.contains_row(row) {
            return Err(ModelError::RowNotFound(row));
        }
        self.index
            .rows
            .by_id
            .get(&row)
            .copied()
            .ok_or(ModelError::RowNotFound(row))
    }

    fn add_column(
        &mut self,
        index: usize,
        name: &str,
        kind: &PropertyType,
    ) -> Result<DatabaseOutcome, ModelError> {
        if let PropertyType::Unknown(name) = kind {
            return Err(ModelError::UnknownPropertyType(name.clone()));
        }
        let count = self.column_ids().len();
        if index > count {
            return Err(ModelError::IndexOutOfRange { index, len: count });
        }
        let column = ColumnId::new();
        let schema = self
            .schema()
            .insert_container(&column.to_string(), LoroMap::new())?;
        schema.insert(keys::NAME, name)?;
        schema.insert(keys::TYPE, kind.type_name())?;
        // Made now, whatever the type, so a later retype to select finds it and undo and redo of options act
        // on a list that was always there.
        schema.insert_container(keys::OPTIONS, LoroMovableList::new())?;
        let live: Vec<TreeID> = self.live_columns().into_iter().map(|(_, n)| n).collect();
        let node = self
            .columns
            .create_at(TreeParentId::Root, tree_position(&self.columns, &live, index))?;
        self.columns.get_meta(node)?.insert(ID, column.to_string())?;
        self.index.columns.insert(column, node);
        Ok(DatabaseOutcome::ColumnCreated(column))
    }

    /// Writes one plain value into a column's schema, if it differs.
    fn set_schema(
        &mut self,
        column: ColumnId,
        key: &str,
        new: LoroValue,
    ) -> Result<DatabaseOutcome, ModelError> {
        self.column(column)?;
        let schema = self.schema_of(column).ok_or(ModelError::ColumnNotFound(column))?;
        if value(&schema, key).as_ref() != Some(&new) {
            schema.insert(key, new)?;
        }
        Ok(DatabaseOutcome::Done)
    }

    fn set_type(&mut self, column: ColumnId, kind: &PropertyType) -> Result<DatabaseOutcome, ModelError> {
        let current = self.column(column)?;
        refuse_unknown(column, &current.kind, kind)?;
        self.set_schema(column, keys::TYPE, kind.type_name().into())
    }

    fn set_width(&mut self, column: ColumnId, width: Option<f64>) -> Result<DatabaseOutcome, ModelError> {
        self.column(column)?;
        match width {
            Some(w) if !(w.is_finite() && w > 0.0) => Err(ModelError::CellTypeMismatch {
                column,
                expected: "a positive width".into(),
            }),
            Some(w) => self.set_schema(column, keys::WIDTH, w.into()),
            None => {
                let schema = self.schema_of(column).ok_or(ModelError::ColumnNotFound(column))?;
                if schema.get(keys::WIDTH).is_some() {
                    schema.delete(keys::WIDTH)?;
                }
                Ok(DatabaseOutcome::Done)
            }
        }
    }

    /// Deletes the column's schema, which is what makes it a column. Its node stays in the order tree (a move on
    /// another device must not bring it back) and its cells stay too (ADR-066), unread.
    fn delete_column(&mut self, column: ColumnId) -> Result<DatabaseOutcome, ModelError> {
        self.column_node(column)?;
        self.schema().delete(&column.to_string())?;
        Ok(DatabaseOutcome::Done)
    }

    /// Adds a row; a row deleted earlier comes back (its node and cells were kept) at the new place.
    fn add_row(&mut self, row: PageId, index: usize) -> Result<DatabaseOutcome, ModelError> {
        if self.contains_row(row) {
            return Err(ModelError::DuplicateRow(row));
        }
        let live: Vec<TreeID> = self.live_rows().into_iter().map(|(_, n)| n).collect();
        if index > live.len() {
            return Err(ModelError::IndexOutOfRange {
                index,
                len: live.len(),
            });
        }
        if let Some(&node) = self.index.rows.by_id.get(&row) {
            self.rows.get_meta(node)?.delete(super::DELETED)?;
            move_node(&self.rows, &live, node, index)?;
            return Ok(DatabaseOutcome::Done);
        }
        let node = self
            .rows
            .create_at(TreeParentId::Root, tree_position(&self.rows, &live, index))?;
        self.rows.get_meta(node)?.insert(ID, row.to_string())?;
        self.index.rows.insert(row, node);
        Ok(DatabaseOutcome::Done)
    }

    /// Clears a deleted row's mark, leaving it where it was. Fails for a row that is live or never existed.
    fn restore_row(&mut self, row: PageId) -> Result<DatabaseOutcome, ModelError> {
        if self.contains_row(row) {
            return Err(ModelError::DuplicateRow(row));
        }
        let node = self
            .index
            .rows
            .by_id
            .get(&row)
            .copied()
            .ok_or(ModelError::RowNotFound(row))?;
        self.rows.get_meta(node)?.delete(super::DELETED)?;
        Ok(DatabaseOutcome::Done)
    }

    fn set_cell(
        &mut self,
        row: PageId,
        column: ColumnId,
        new: Option<&CellValue>,
    ) -> Result<DatabaseOutcome, ModelError> {
        if !self.contains_row(row) {
            return Err(ModelError::RowNotFound(row));
        }
        let current = self.column(column)?;
        if let PropertyType::Unknown(type_name) = &current.kind {
            return Err(ModelError::UnknownColumnType {
                column,
                type_name: type_name.clone(),
            });
        }
        if let Some(new) = new {
            if !new.fits(&current.kind) {
                return Err(ModelError::CellTypeMismatch {
                    column,
                    expected: current.kind.type_name().to_owned(),
                });
            }
            check_options(&current, new)?;
        }
        if self.cell(row, column)?.as_ref() == new {
            return Ok(DatabaseOutcome::Unchanged);
        }
        // Mergeable, so two devices filling in a row for the first time at once share one map (E09-S01).
        let cells = self.cells().ensure_mergeable_map(&row.to_string())?;
        let key = column.to_string();
        // A multi-select's map is mergeable: attached again after being taken out, it would be the same container
        // with whatever it held then, which undo and redo don't follow. So once a cell has a map it keeps it: a
        // plain value goes inside it (`keys::PLAIN`) and clearing empties it (E09-S12).
        let stored_map = match cells.get(&key) {
            Some(ValueOrContainer::Container(Container::Map(map))) => Some(map),
            _ => None,
        };
        match new {
            Some(CellValue::Options(options)) => write_options(&cells.ensure_mergeable_map(&key)?, options)?,
            // A multi-select is emptied option by option, so one added elsewhere at the same time is kept.
            None if current.kind == PropertyType::MultiSelect => {
                write_options(
                    &cells.ensure_mergeable_map(&key)?,
                    &std::collections::BTreeSet::new(),
                )?;
            }
            Some(value) => {
                if let Some(plain) = value.to_loro() {
                    match &stored_map {
                        Some(map) => {
                            write_options(map, &std::collections::BTreeSet::new())?;
                            map.insert(keys::PLAIN, plain)?;
                        }
                        None => cells.insert(&key, plain)?,
                    }
                }
            }
            None => match &stored_map {
                Some(map) => write_options(map, &std::collections::BTreeSet::new())?,
                None => cells.delete(&key)?,
            },
        }
        Ok(DatabaseOutcome::Done)
    }

    fn set_view(
        &mut self,
        key: &str,
        new: LoroValue,
        current: &LoroValue,
    ) -> Result<DatabaseOutcome, ModelError> {
        if &new != current {
            self.view().insert(key, new)?;
        }
        Ok(DatabaseOutcome::Done)
    }
}

/// A column of an unknown type can't be changed into anything, nor anything into one: this version doesn't
/// know what such columns hold.
fn refuse_unknown(column: ColumnId, current: &PropertyType, kind: &PropertyType) -> Result<(), ModelError> {
    match (current, kind) {
        (PropertyType::Unknown(name), _) | (_, PropertyType::Unknown(name)) => {
            Err(ModelError::UnknownColumnType {
                column,
                type_name: name.clone(),
            })
        }
        _ => Ok(()),
    }
}

/// Where in the tree's top-level nodes (deleted ones included) a new node goes to be the `index`th live one.
fn tree_position(tree: &LoroTree, live: &[TreeID], index: usize) -> usize {
    let roots = tree.roots();
    live.get(index)
        .and_then(|node| roots.iter().position(|n| n == node))
        .unwrap_or(roots.len())
}

/// Moves `node` to be the `index`th of the `live` nodes, counted without it.
fn move_node(
    tree: &LoroTree,
    live: &[TreeID],
    node: TreeID,
    index: usize,
) -> Result<DatabaseOutcome, ModelError> {
    let others: Vec<TreeID> = live.iter().copied().filter(|&n| n != node).collect();
    if index > others.len() {
        return Err(ModelError::IndexOutOfRange {
            index,
            len: others.len(),
        });
    }
    if live.iter().position(|&n| n == node) == Some(index) {
        return Ok(DatabaseOutcome::Unchanged);
    }
    match (others.get(index), others.last()) {
        (Some(&before), _) => tree.mov_before(node, before)?,
        (None, Some(&after)) => tree.mov_after(node, after)?,
        (None, None) => return Ok(DatabaseOutcome::Unchanged),
    }
    Ok(DatabaseOutcome::Done)
}
