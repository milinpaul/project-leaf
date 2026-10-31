//! The column menu (E09-S06): what a column's header menu offers, and what each choice writes. Adding, renaming,
//! retyping, moving, hiding, showing, resizing and deleting a column are each one `DatabaseCommand`, so each is one
//! undo step. The UI draws the menus and forwards; every decision is here.
//!
//! - Moving works among the shown columns: "left" and "right" step past the next shown column, and a header dragged
//!   from one shown position to another lands next to the column it was dropped on. Hidden columns keep their place.
//! - A new column is named after its type ("Text", then "Text 2", …), so its name never clashes.
//! - Renaming to blank text keeps the old name.
//! - Changing the type keeps every stored value; the table converts them for display (ADR 0066 answer 3).

use leaf_core::ColumnId;
use leaf_model::{Column, DatabaseCommand, DatabaseOutcome, ModelError, PropertyType};

use super::DatabaseEditor;

/// The types a column can be given, in the order the menus list them.
pub const COLUMN_TYPES: [PropertyType; 7] = [
    PropertyType::Text,
    PropertyType::Number,
    PropertyType::Select,
    PropertyType::MultiSelect,
    PropertyType::Date,
    PropertyType::Checkbox,
    PropertyType::Url,
];

/// A type's name in the menus.
#[must_use]
pub fn type_label(kind: &PropertyType) -> &'static str {
    match kind {
        PropertyType::Text => "Text",
        PropertyType::Number => "Number",
        PropertyType::Select => "Select",
        PropertyType::MultiSelect => "Multi-select",
        PropertyType::Date => "Date",
        PropertyType::Checkbox => "Checkbox",
        PropertyType::Url => "URL",
        PropertyType::Unknown(_) => "Unknown",
    }
}

/// A column's header menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMenu {
    /// The column.
    pub column: ColumnId,
    /// Its name.
    pub name: String,
    /// Its type.
    pub kind: PropertyType,
    /// Whether its type can be changed: not for a type from a newer version, which this one can't write.
    pub retypable: bool,
    /// Whether a shown column is to its left.
    pub can_move_left: bool,
    /// Whether a shown column is to its right.
    pub can_move_right: bool,
}

/// A hidden column, listed so it can be shown again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenColumn {
    /// The column.
    pub column: ColumnId,
    /// Its name.
    pub name: String,
    /// Its type, for its icon.
    pub kind: PropertyType,
}

impl DatabaseEditor {
    fn all_columns(&self) -> Result<Vec<Column>, ModelError> {
        self.db.columns()
    }

    /// The full positions of the shown columns, in order.
    fn shown_positions(columns: &[Column]) -> Vec<usize> {
        columns
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.hidden)
            .map(|(i, _)| i)
            .collect()
    }

    /// The header menu of `column`.
    pub fn column_menu(&self, column: ColumnId) -> Result<ColumnMenu, ModelError> {
        let columns = self.all_columns()?;
        let shown = Self::shown_positions(&columns);
        let at = columns
            .iter()
            .position(|c| c.id == column)
            .ok_or(ModelError::ColumnNotFound(column))?;
        let found = &columns[at];
        Ok(ColumnMenu {
            column,
            name: found.name.clone(),
            kind: found.kind.clone(),
            retypable: !matches!(found.kind, PropertyType::Unknown(_)),
            can_move_left: shown.iter().any(|&i| i < at),
            can_move_right: shown.iter().any(|&i| i > at),
        })
    }

    /// The hidden columns, in column order.
    pub fn hidden_columns(&self) -> Result<Vec<HiddenColumn>, ModelError> {
        Ok(self
            .all_columns()?
            .into_iter()
            .filter(|c| c.hidden)
            .map(|c| HiddenColumn {
                column: c.id,
                name: c.name,
                kind: c.kind,
            })
            .collect())
    }

    /// Adds a column of `kind` after the others, named after its type, and returns it.
    pub fn add_column(&mut self, kind: PropertyType) -> Result<ColumnId, ModelError> {
        let columns = self.all_columns()?;
        let base = type_label(&kind);
        let taken = |name: &str| columns.iter().any(|c| c.name.eq_ignore_ascii_case(name));
        // One more candidate than there are columns: at least one is free.
        let name = (1..=columns.len() + 1)
            .map(|n| {
                if n == 1 {
                    base.to_owned()
                } else {
                    format!("{base} {n}")
                }
            })
            .find(|name| !taken(name))
            .unwrap_or_else(|| base.to_owned());
        match self.db.apply(DatabaseCommand::AddColumn {
            index: columns.len(),
            name,
            kind,
        })? {
            DatabaseOutcome::ColumnCreated(column) => Ok(column),
            _ => unreachable!("AddColumn always reports the column it created"),
        }
    }

    /// Renames `column` to the trimmed `name`. Returns `false` (and keeps the old name) for blank text.
    pub fn rename_column(&mut self, column: ColumnId, name: &str) -> Result<bool, ModelError> {
        let name = name.trim();
        if name.is_empty() {
            return Ok(false);
        }
        self.db.apply(DatabaseCommand::RenameColumn {
            column,
            name: name.to_owned(),
        })?;
        Ok(true)
    }

    /// Changes `column`'s type. Its stored values stay as they are.
    pub fn set_column_type(&mut self, column: ColumnId, kind: PropertyType) -> Result<(), ModelError> {
        self.db.apply(DatabaseCommand::SetColumnType { column, kind })?;
        Ok(())
    }

    /// Moves `column` one shown column to the left (`left`) or right. Returns `false` when there is none that way.
    pub fn step_column(&mut self, column: ColumnId, left: bool) -> Result<bool, ModelError> {
        let columns = self.all_columns()?;
        let shown = Self::shown_positions(&columns);
        let at = columns
            .iter()
            .position(|c| c.id == column)
            .ok_or(ModelError::ColumnNotFound(column))?;
        let neighbour = if left {
            shown.iter().rev().find(|&&i| i < at)
        } else {
            shown.iter().find(|&&i| i > at)
        };
        let Some(&index) = neighbour else {
            return Ok(false);
        };
        // Counted without the moved column, the neighbour's own position puts it on the neighbour's far side.
        self.db.apply(DatabaseCommand::MoveColumn { column, index })?;
        Ok(true)
    }

    /// Moves the shown column at shown position `from` to shown position `to` (a header dragged and dropped).
    pub fn move_shown_column(&mut self, from: usize, to: usize) -> Result<bool, ModelError> {
        let columns = self.all_columns()?;
        let shown = Self::shown_positions(&columns);
        let (Some(&at), Some(&target)) = (shown.get(from), shown.get(to)) else {
            return Ok(false);
        };
        if from == to {
            return Ok(false);
        }
        self.db.apply(DatabaseCommand::MoveColumn {
            column: columns[at].id,
            index: target,
        })?;
        Ok(true)
    }

    /// Hides or shows `column` in the table.
    pub fn set_column_hidden(&mut self, column: ColumnId, hidden: bool) -> Result<(), ModelError> {
        self.db
            .apply(DatabaseCommand::SetColumnHidden { column, hidden })?;
        Ok(())
    }

    /// Sets `column`'s width in points, as dragged. Returns `false` when it hasn't changed.
    pub fn set_column_width(&mut self, column: ColumnId, width: f64) -> Result<bool, ModelError> {
        let current = self
            .db
            .column(column)?
            .width
            .unwrap_or(super::DEFAULT_COLUMN_WIDTH);
        if (current - width).abs() < 0.5 || !width.is_finite() || width <= 0.0 {
            return Ok(false);
        }
        self.db.apply(DatabaseCommand::SetColumnWidth {
            column,
            width: Some(width.round()),
        })?;
        Ok(true)
    }

    /// Deletes `column`. Its values stay in the document, so undo brings it back whole.
    pub fn delete_column(&mut self, column: ColumnId) -> Result<(), ModelError> {
        self.db.apply(DatabaseCommand::DeleteColumn { column })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
