//! Editing a table's cells in place (E09-S05): what kind of editor each cell opens, and what each kind of input
//! writes. The UI draws the editors and forwards what the person does; every decision is here.
//!
//! - Text, URL and number cells edit as a line of text. Committing writes the text (a number must read as one;
//!   empty text empties the cell).
//! - A checkbox toggles; it never opens an editor.
//! - Select and multi-select cells open a picker: choosing an option sets it (select) or adds or removes it
//!   (multi-select), and typing a name that isn't there offers to create it, in the next colour of the palette.
//! - Date cells open a date picker, which sets or clears the date.
//! - Columns of a type from a newer version are read-only.

use leaf_core::{ColumnId, OptionId, PageId};
use leaf_model::{CellValue, Column, DatabaseCommand, DatabaseOutcome, ModelError, PropertyType};

use super::DatabaseEditor;
use super::display::{as_number, as_text};
use crate::fuzzy_match;

/// Colours new options get, in turn (the theme draws each; see `leaf-ui`).
pub const OPTION_COLORS: [&str; 9] = [
    "gray", "brown", "orange", "yellow", "green", "blue", "purple", "pink", "red",
];

/// How a cell edits.
#[derive(Debug, Clone, PartialEq)]
pub enum CellEditor {
    /// A line of text, starting from `initial`: text, URL and number cells.
    Text {
        /// What the field starts with.
        initial: String,
        /// Whether what is typed must read as a number.
        number: bool,
    },
    /// A checkbox: toggled, no editor.
    Checkbox,
    /// The option picker.
    Options {
        /// Whether several options can be picked.
        multiple: bool,
    },
    /// The date picker, starting from the cell's date.
    Date {
        /// The cell's date, unix milliseconds.
        initial: Option<i64>,
    },
    /// Not editable here.
    ReadOnly,
}

/// One entry of the option picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionChoice {
    /// The option.
    pub option: OptionId,
    /// Its name.
    pub name: String,
    /// Its colour name.
    pub color: String,
    /// Whether the cell has it.
    pub picked: bool,
}

/// The option picker's contents for a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionPicker {
    /// Matching options: all of them, in the column's order, for an empty query; best matches first otherwise.
    pub choices: Vec<OptionChoice>,
    /// The name a "Create" entry would make: the trimmed query, when no option has exactly that name.
    pub create: Option<String>,
}

/// What a text commit did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCommit {
    /// The cell now holds the text (or is empty).
    Saved,
    /// The text doesn't read as a number: nothing was written, and the editor stays open.
    NotANumber,
}

impl DatabaseEditor {
    fn column_of(&self, column: ColumnId) -> Result<Column, ModelError> {
        self.db.column(column)
    }

    /// How the cell at `row`, `column` edits.
    pub fn cell_editor(&self, row: PageId, column: ColumnId) -> Result<CellEditor, ModelError> {
        let column = self.column_of(column)?;
        let value = self.db.cell(row, column.id)?;
        Ok(match column.kind {
            PropertyType::Text | PropertyType::Url => CellEditor::Text {
                initial: value.and_then(|v| as_text(&v, &column, 0)).unwrap_or_default(),
                number: false,
            },
            PropertyType::Number => CellEditor::Text {
                initial: value
                    .and_then(|v| as_number(&v))
                    .map(|n| format!("{n}"))
                    .unwrap_or_default(),
                number: true,
            },
            PropertyType::Checkbox => CellEditor::Checkbox,
            PropertyType::Select => CellEditor::Options { multiple: false },
            PropertyType::MultiSelect => CellEditor::Options { multiple: true },
            PropertyType::Date => CellEditor::Date {
                initial: match value {
                    Some(CellValue::Date(at)) => Some(at),
                    _ => None,
                },
            },
            PropertyType::Unknown(_) => CellEditor::ReadOnly,
        })
    }

    /// Writes typed text into a text, URL or number cell. Empty (or blank) text empties the cell.
    pub fn commit_text(
        &mut self,
        row: PageId,
        column: ColumnId,
        text: &str,
    ) -> Result<TextCommit, ModelError> {
        let kind = self.column_of(column)?.kind;
        let trimmed = text.trim();
        let value = match kind {
            _ if trimmed.is_empty() => None,
            PropertyType::Number => match trimmed.parse::<f64>() {
                Ok(n) if n.is_finite() => Some(CellValue::Number(n)),
                _ => return Ok(TextCommit::NotANumber),
            },
            PropertyType::Url => Some(CellValue::Text(trimmed.to_owned())),
            _ => Some(CellValue::Text(text.to_owned())),
        };
        self.db.apply(DatabaseCommand::SetCell { row, column, value })?;
        Ok(TextCommit::Saved)
    }

    /// Ticks or unticks a checkbox cell.
    pub fn toggle_checkbox(&mut self, row: PageId, column: ColumnId) -> Result<(), ModelError> {
        let on = matches!(self.db.cell(row, column)?, Some(CellValue::Checkbox(true)));
        self.db.apply(DatabaseCommand::SetCell {
            row,
            column,
            value: Some(CellValue::Checkbox(!on)),
        })?;
        Ok(())
    }

    /// The options the cell's picker shows for `query`.
    pub fn option_picker(
        &self,
        row: PageId,
        column: ColumnId,
        query: &str,
    ) -> Result<OptionPicker, ModelError> {
        let column = self.column_of(column)?;
        let picked: Vec<OptionId> = match self.db.cell(row, column.id)? {
            Some(CellValue::Option(id)) => vec![id],
            Some(CellValue::Options(ids)) => ids.into_iter().collect(),
            _ => Vec::new(),
        };
        let query = query.trim();
        let mut scored: Vec<(i64, usize, OptionChoice)> = column
            .options
            .iter()
            .enumerate()
            .filter_map(|(order, option)| {
                let score = if query.is_empty() {
                    0
                } else {
                    i64::from(fuzzy_match(&option.name, query)?.score)
                };
                Some((
                    score,
                    order,
                    OptionChoice {
                        option: option.id,
                        name: option.name.clone(),
                        color: option.color.clone(),
                        picked: picked.contains(&option.id),
                    },
                ))
            })
            .collect();
        scored.sort_by_key(|(score, order, _)| (std::cmp::Reverse(*score), *order));
        let exists = column.options.iter().any(|o| o.name.eq_ignore_ascii_case(query));
        Ok(OptionPicker {
            choices: scored.into_iter().map(|(_, _, choice)| choice).collect(),
            create: (!query.is_empty() && !exists).then(|| query.to_owned()),
        })
    }

    /// Picks `option` in the cell: a select cell takes it (or lets it go, if it had it); a multi-select cell adds
    /// it or takes it away.
    pub fn toggle_option(
        &mut self,
        row: PageId,
        column: ColumnId,
        option: OptionId,
    ) -> Result<(), ModelError> {
        let kind = self.column_of(column)?.kind;
        let current = self.db.cell(row, column)?;
        let value = match kind {
            PropertyType::Select => match current {
                Some(CellValue::Option(id)) if id == option => None,
                _ => Some(CellValue::Option(option)),
            },
            PropertyType::MultiSelect => {
                let mut options = match current {
                    Some(CellValue::Options(ids)) => ids,
                    _ => std::collections::BTreeSet::new(),
                };
                if !options.remove(&option) {
                    options.insert(option);
                }
                Some(CellValue::Options(options))
            }
            _ => return Err(ModelError::NotASelectColumn(column)),
        };
        self.db.apply(DatabaseCommand::SetCell { row, column, value })?;
        Ok(())
    }

    /// Adds an option named `name` to the column, in the next palette colour, and picks it in the cell. Returns
    /// the new option.
    pub fn create_option(
        &mut self,
        row: PageId,
        column: ColumnId,
        name: &str,
    ) -> Result<OptionId, ModelError> {
        let count = self.column_of(column)?.options.len();
        let color = OPTION_COLORS[count % OPTION_COLORS.len()].to_owned();
        let DatabaseOutcome::OptionCreated(option) = self.db.apply(DatabaseCommand::AddOption {
            column,
            name: name.trim().to_owned(),
            color,
        })?
        else {
            unreachable!("AddOption always reports the option it created");
        };
        self.toggle_option(row, column, option)?;
        Ok(option)
    }

    /// Sets or clears a date cell.
    pub fn set_date(&mut self, row: PageId, column: ColumnId, at: Option<i64>) -> Result<(), ModelError> {
        self.db.apply(DatabaseCommand::SetCell {
            row,
            column,
            value: at.map(CellValue::Date),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
