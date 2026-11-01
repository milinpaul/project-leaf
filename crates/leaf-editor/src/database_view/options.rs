//! Editing a select or multi-select column's options (E09-S11): what an option's menu shows, and what renaming,
//! recolouring, moving and deleting one writes. The UI draws the menu and the picker's dragging, and forwards.
//!
//! - A name is trimmed; a blank one, or one another option of the column already has (any case), keeps the old name,
//!   so two options never read the same.
//! - Colours are the nine of [`OPTION_COLORS`]; an option keeps a colour from a newer version until another is picked.
//! - Deleting an option takes it out of every cell that has it, then off the list, as one undo step: undo brings both
//!   back.

use leaf_core::{ColumnId, OptionId};
use leaf_model::{CellValue, Column, Database, DatabaseCommand, DatabaseOutcome, ModelError, SelectOption};

use super::{DatabaseEditor, OPTION_COLORS};

/// An option's menu: its name, its colour and the colours it can be given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionMenu {
    /// The column.
    pub column: ColumnId,
    /// The option.
    pub option: OptionId,
    /// Its name.
    pub name: String,
    /// Its colour name.
    pub color: String,
    /// The palette, in order, each with whether it is the option's colour.
    pub colors: Vec<(&'static str, bool)>,
}

impl DatabaseEditor {
    /// The menu of `option` in `column`.
    pub fn option_menu(&self, column: ColumnId, option: OptionId) -> Result<OptionMenu, ModelError> {
        let found = find(&self.db.column(column)?, option)?;
        Ok(OptionMenu {
            column,
            option,
            colors: OPTION_COLORS.iter().map(|&c| (c, c == found.color)).collect(),
            name: found.name,
            color: found.color,
        })
    }

    /// Renames `option` to `name`, trimmed. Returns whether it changed: a blank name, the same name, or one another
    /// option has keeps the old one.
    pub fn rename_option(
        &mut self,
        column: ColumnId,
        option: OptionId,
        name: &str,
    ) -> Result<bool, ModelError> {
        let current = self.db.column(column)?;
        let old = find(&current, option)?;
        let name = name.trim();
        let taken = current
            .options
            .iter()
            .any(|o| o.id != option && o.name.eq_ignore_ascii_case(name));
        if name.is_empty() || name == old.name || taken {
            return Ok(false);
        }
        self.db.apply(DatabaseCommand::RenameOption {
            column,
            option,
            name: name.to_owned(),
        })?;
        Ok(true)
    }

    /// Gives `option` the palette colour `color`. Returns whether it changed: a name outside the palette is refused.
    pub fn set_option_color(
        &mut self,
        column: ColumnId,
        option: OptionId,
        color: &str,
    ) -> Result<bool, ModelError> {
        let old = find(&self.db.column(column)?, option)?;
        if !OPTION_COLORS.contains(&color) || old.color == color {
            return Ok(false);
        }
        self.db.apply(DatabaseCommand::SetOptionColor {
            column,
            option,
            color: color.to_owned(),
        })?;
        Ok(true)
    }

    /// Moves `option` to position `index` among the column's options (dragged in the picker). Returns whether it moved.
    pub fn move_option(
        &mut self,
        column: ColumnId,
        option: OptionId,
        index: usize,
    ) -> Result<bool, ModelError> {
        let outcome = self.db.apply(DatabaseCommand::MoveOption {
            column,
            option,
            index,
        })?;
        Ok(outcome != DatabaseOutcome::Unchanged)
    }

    /// Deletes `option`: every cell that has it loses it, then it leaves the list. One undo step.
    pub fn delete_option(&mut self, column: ColumnId, option: OptionId) -> Result<(), ModelError> {
        find(&self.db.column(column)?, option)?;
        self.db.group(|db| {
            clear_from_cells(db, column, option)?;
            db.apply(DatabaseCommand::DeleteOption { column, option })?;
            Ok(())
        })?;
        Ok(())
    }
}

/// `option` in `column`, or an error naming it.
fn find(column: &Column, option: OptionId) -> Result<SelectOption, ModelError> {
    column
        .options
        .iter()
        .find(|o| o.id == option)
        .cloned()
        .ok_or(ModelError::OptionNotFound {
            column: column.id,
            option,
        })
}

/// Takes `option` out of every row's cell in `column`.
fn clear_from_cells(db: &mut Database, column: ColumnId, option: OptionId) -> Result<(), ModelError> {
    for row in db.rows() {
        let value = match db.cell(row, column)? {
            Some(CellValue::Option(id)) if id == option => None,
            Some(CellValue::Options(ids)) if ids.contains(&option) => {
                let rest: std::collections::BTreeSet<OptionId> =
                    ids.into_iter().filter(|&id| id != option).collect();
                (!rest.is_empty()).then_some(CellValue::Options(rest))
            }
            _ => continue,
        };
        db.apply(DatabaseCommand::SetCell { row, column, value })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
