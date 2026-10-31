//! Sorting and filtering a table (E09-S07): which conditions each type offers, what the bar above the table shows,
//! and what each change writes. The view applies them (`order.rs`); this decides how they are edited.
//!
//! - Each change is one `SetSort` or `SetFilters` command, so one undo step, saved and synced with the database.
//! - A filter whose value is typed (text, URL, number) is a *draft* while typing: the table shows its rows at
//!   once, and the database gets it when the typing is committed (Enter, or closing the filter). So typing a word
//!   is one undo step, not one per letter.
//! - A new filter starts with its type's first condition and no value, which narrows nothing until a value is
//!   given. A checkbox filter starts as "is ticked".
//! - Choosing "is empty" or "is not empty" drops the value; switching between conditions that both take a value
//!   keeps it.

use leaf_core::{ColumnId, OptionId};
use leaf_model::{CellValue, Column, DatabaseCommand, Filter, FilterOp, ModelError, PropertyType, Sort};

use super::DatabaseEditor;
use super::display::{as_number, date_label};

/// The conditions a column of `kind` can be filtered by, in the order the menu lists them. The first is the one
/// a new filter starts with.
#[must_use]
pub fn filter_ops(kind: &PropertyType) -> Vec<FilterOp> {
    use FilterOp::{Contains, DoesNotContain, Greater, Is, IsEmpty, IsNot, IsNotEmpty, Less};
    match kind {
        PropertyType::Text | PropertyType::Url => {
            vec![Contains, DoesNotContain, Is, IsNot, IsEmpty, IsNotEmpty]
        }
        PropertyType::Number => vec![Is, IsNot, Greater, Less, IsEmpty, IsNotEmpty],
        PropertyType::Select => vec![Is, IsNot, IsEmpty, IsNotEmpty],
        PropertyType::MultiSelect => vec![Contains, DoesNotContain, IsEmpty, IsNotEmpty],
        PropertyType::Date => vec![Is, Less, Greater, IsEmpty, IsNotEmpty],
        PropertyType::Checkbox => vec![Is],
        PropertyType::Unknown(_) => Vec::new(),
    }
}

/// How a condition reads for a column of `kind`: "contains", "≠", "is before", "has".
#[must_use]
pub fn op_label(op: &FilterOp, kind: &PropertyType) -> &'static str {
    match (op, kind) {
        (FilterOp::Is, PropertyType::Number) => "=",
        (FilterOp::IsNot, PropertyType::Number) => "≠",
        (FilterOp::Greater, PropertyType::Number) => ">",
        (FilterOp::Less, PropertyType::Number) => "<",
        (FilterOp::Greater, _) => "is after",
        (FilterOp::Less, _) => "is before",
        (FilterOp::Contains, PropertyType::MultiSelect) => "has",
        (FilterOp::DoesNotContain, PropertyType::MultiSelect) => "does not have",
        (FilterOp::Is, _) => "is",
        (FilterOp::IsNot, _) => "is not",
        (FilterOp::Contains, _) => "contains",
        (FilterOp::DoesNotContain, _) => "does not contain",
        (FilterOp::IsEmpty, _) => "is empty",
        (FilterOp::IsNotEmpty, _) => "is not empty",
        (FilterOp::Unknown(_), _) => "unknown condition",
    }
}

/// Whether a condition compares with a value.
fn takes_value(op: &FilterOp) -> bool {
    !matches!(
        op,
        FilterOp::IsEmpty | FilterOp::IsNotEmpty | FilterOp::Unknown(_)
    )
}

/// What a filter's value is edited with.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterInput {
    /// Nothing: the condition takes no value ("is empty"), or comes from a newer version.
    None,
    /// A line of text (text and URL columns), as typed.
    Text(String),
    /// A line of text that must read as a number.
    Number(String),
    /// One option of the column, picked from its list.
    Option(Option<OptionId>),
    /// A date, from a calendar (unix milliseconds).
    Date(Option<i64>),
    /// Ticked or not.
    Checkbox(bool),
}

/// One sort, as the bar shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortChip {
    /// The column it sorts by.
    pub column: ColumnId,
    /// The column's name.
    pub name: String,
    /// The column's type, for its icon.
    pub kind: PropertyType,
    /// Largest, latest or Z first.
    pub descending: bool,
}

/// One filter, as the bar shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterChip {
    /// The column it looks at.
    pub column: ColumnId,
    /// The column's name.
    pub name: String,
    /// The column's type.
    pub kind: PropertyType,
    /// Its condition.
    pub op: FilterOp,
    /// The condition as it reads ("is before").
    pub op_label: &'static str,
    /// Its value as it reads ("Doing", "12 Oct 2026", "“seed”"); empty when it has none yet.
    pub value_label: String,
    /// The conditions it could switch to.
    pub ops: Vec<FilterOp>,
    /// How its value is edited.
    pub input: FilterInput,
}

/// The bar above the table: the sorts, the filters, and the columns each can still be added for.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryBar {
    /// The sorts, first key first.
    pub sorts: Vec<SortChip>,
    /// The filters, in order (a typed one as typed).
    pub filters: Vec<FilterChip>,
    /// Columns a sort can be added for: not already sorted by, and of a type this version knows.
    pub sortable: Vec<(ColumnId, String, PropertyType)>,
    /// Columns a filter can be added for: of a type this version knows.
    pub filterable: Vec<(ColumnId, String, PropertyType)>,
}

impl DatabaseEditor {
    /// The filters as the table applies them: the database's, with the draft in place of the one being typed.
    pub(super) fn filters(&self) -> Vec<Filter> {
        let mut filters = self.db.filters();
        if let Some((index, draft)) = &self.draft
            && let Some(slot) = filters.get_mut(*index)
        {
            *slot = draft.clone();
        }
        filters
    }

    /// What the bar above the table shows. `offset_minutes` is the device's time zone, for dates.
    pub fn query_bar(&self, offset_minutes: i32) -> Result<QueryBar, ModelError> {
        let columns = self.db.columns()?;
        let find = |id: ColumnId| columns.iter().find(|c| c.id == id);
        let sorts: Vec<SortChip> = self
            .db
            .sort()
            .into_iter()
            .filter_map(|s| {
                find(s.column).map(|c| SortChip {
                    column: c.id,
                    name: c.name.clone(),
                    kind: c.kind.clone(),
                    descending: s.descending,
                })
            })
            .collect();
        let filters = self
            .filters()
            .into_iter()
            .filter_map(|f| find(f.column).map(|c| chip(&f, c, offset_minutes)))
            .collect();
        let known = |c: &&Column| !matches!(c.kind, PropertyType::Unknown(_));
        let entry = |c: &Column| (c.id, c.name.clone(), c.kind.clone());
        Ok(QueryBar {
            sortable: columns
                .iter()
                .filter(known)
                .filter(|c| !sorts.iter().any(|s| s.column == c.id))
                .map(entry)
                .collect(),
            filterable: columns.iter().filter(known).map(entry).collect(),
            sorts,
            filters,
        })
    }

    fn write_sort(&mut self, sort: Vec<Sort>) -> Result<(), ModelError> {
        self.db.apply(DatabaseCommand::SetSort(sort))?;
        Ok(())
    }

    /// Adds a sort by `column`, A to Z, after the others. Returns `false` when the table already sorts by it.
    pub fn add_sort(&mut self, column: ColumnId) -> Result<bool, ModelError> {
        self.db.column(column)?;
        let mut sort = self.db.sort();
        if sort.iter().any(|s| s.column == column) {
            return Ok(false);
        }
        sort.push(Sort::new(column, false));
        self.write_sort(sort)?;
        Ok(true)
    }

    /// Turns sort `index` the other way round (or sets it to `descending`).
    pub fn set_sort_descending(&mut self, index: usize, descending: bool) -> Result<(), ModelError> {
        let mut sort = self.db.sort();
        let len = sort.len();
        let entry = sort
            .get_mut(index)
            .ok_or(ModelError::IndexOutOfRange { index, len })?;
        if entry.descending == descending {
            return Ok(());
        }
        entry.descending = descending;
        self.write_sort(sort)
    }

    /// Removes sort `index`.
    pub fn remove_sort(&mut self, index: usize) -> Result<(), ModelError> {
        let mut sort = self.db.sort();
        if index >= sort.len() {
            return Err(ModelError::IndexOutOfRange {
                index,
                len: sort.len(),
            });
        }
        sort.remove(index);
        self.write_sort(sort)
    }

    /// Writes the filters, ending any draft (it is in `filters` if it should be).
    fn write_filters(&mut self, filters: Vec<Filter>) -> Result<(), ModelError> {
        self.draft = None;
        self.db.apply(DatabaseCommand::SetFilters(filters))?;
        Ok(())
    }

    /// The filters with any draft committed, for a change on top of them.
    fn current_filters(&self) -> Vec<Filter> {
        self.filters()
    }

    /// Adds a filter on `column` with its type's first condition, and returns its position.
    pub fn add_filter(&mut self, column: ColumnId) -> Result<usize, ModelError> {
        let kind = self.db.column(column)?.kind;
        let op = filter_ops(&kind)
            .into_iter()
            .next()
            .ok_or(ModelError::UnknownColumnType {
                column,
                type_name: kind.type_name().to_owned(),
            })?;
        let value = (kind == PropertyType::Checkbox).then_some(CellValue::Checkbox(true));
        let mut filters = self.current_filters();
        filters.push(Filter::new(column, op, value));
        let index = filters.len().saturating_sub(1);
        self.write_filters(filters)?;
        Ok(index)
    }

    fn filter_at(filters: &mut [Filter], index: usize) -> Result<&mut Filter, ModelError> {
        let len = filters.len();
        filters
            .get_mut(index)
            .ok_or(ModelError::IndexOutOfRange { index, len })
    }

    /// Changes filter `index`'s condition. "Is empty" and "is not empty" drop the value; the others keep it.
    pub fn set_filter_op(&mut self, index: usize, op: FilterOp) -> Result<(), ModelError> {
        let mut filters = self.current_filters();
        let filter = Self::filter_at(&mut filters, index)?;
        if filter.op == op {
            return Ok(());
        }
        if !takes_value(&op) {
            filter.value = None;
        }
        filter.op = op;
        self.write_filters(filters)
    }

    /// Sets filter `index`'s value: an option, a date, ticked or not (`None` clears it).
    pub fn set_filter_value(&mut self, index: usize, value: Option<CellValue>) -> Result<(), ModelError> {
        let mut filters = self.current_filters();
        let filter = Self::filter_at(&mut filters, index)?;
        if filter.value == value {
            return Ok(());
        }
        filter.value = value;
        self.write_filters(filters)
    }

    /// Typing into filter `index`'s value: the table shows the result at once, nothing is written yet. Returns
    /// `false` when a number filter's text doesn't read as a number (it then narrows nothing).
    pub fn type_filter_text(&mut self, index: usize, text: &str) -> Result<bool, ModelError> {
        let mut filters = self.db.filters();
        let filter = Self::filter_at(&mut filters, index)?.clone();
        let kind = self.db.column(filter.column)?.kind;
        let trimmed = text.trim();
        let (value, readable) = match kind {
            _ if trimmed.is_empty() => (None, true),
            PropertyType::Number => match trimmed.parse::<f64>() {
                Ok(n) if n.is_finite() => (Some(CellValue::Number(n)), true),
                _ => (None, false),
            },
            _ => (Some(CellValue::Text(trimmed.to_owned())), true),
        };
        let mut draft = filter;
        draft.value = value;
        self.draft = Some((index, draft));
        Ok(readable)
    }

    /// Writes the filter being typed, if any. Returns whether there was one that changed anything.
    pub fn commit_filter_text(&mut self) -> Result<bool, ModelError> {
        let Some((index, draft)) = self.draft.take() else {
            return Ok(false);
        };
        let mut filters = self.db.filters();
        match filters.get_mut(index) {
            Some(slot) if *slot != draft => *slot = draft,
            _ => return Ok(false),
        }
        self.write_filters(filters)?;
        Ok(true)
    }

    /// Drops the filter being typed, leaving the database's as it was.
    pub fn cancel_filter_text(&mut self) {
        self.draft = None;
    }

    /// Removes filter `index`.
    pub fn remove_filter(&mut self, index: usize) -> Result<(), ModelError> {
        let mut filters = self.current_filters();
        if index >= filters.len() {
            return Err(ModelError::IndexOutOfRange {
                index,
                len: filters.len(),
            });
        }
        filters.remove(index);
        self.write_filters(filters)
    }
}

/// A filter as the bar shows it, for its column.
fn chip(filter: &Filter, column: &Column, offset_minutes: i32) -> FilterChip {
    let value = if takes_value(&filter.op) {
        filter.value.as_ref()
    } else {
        None
    };
    let option_name = |id: OptionId| column.options.iter().find(|o| o.id == id).map(|o| o.name.clone());
    let picked = match value {
        Some(CellValue::Option(id)) => Some(*id),
        Some(CellValue::Options(ids)) => ids.iter().next().copied(),
        _ => None,
    };
    let (input, value_label) = match (&column.kind, takes_value(&filter.op)) {
        (_, false) | (PropertyType::Unknown(_), true) => (FilterInput::None, String::new()),
        (PropertyType::Text | PropertyType::Url, true) => {
            let text = match value {
                Some(CellValue::Text(t)) => t.clone(),
                _ => String::new(),
            };
            let label = if text.is_empty() {
                String::new()
            } else {
                format!("\u{201c}{text}\u{201d}")
            };
            (FilterInput::Text(text), label)
        }
        (PropertyType::Number, true) => {
            let text = value
                .and_then(as_number)
                .map(|n| format!("{n}"))
                .unwrap_or_default();
            (FilterInput::Number(text.clone()), text)
        }
        (PropertyType::Select | PropertyType::MultiSelect, true) => (
            FilterInput::Option(picked),
            picked.and_then(option_name).unwrap_or_default(),
        ),
        (PropertyType::Date, true) => {
            let at = match value {
                Some(CellValue::Date(at)) => Some(*at),
                _ => None,
            };
            (
                FilterInput::Date(at),
                at.map(|at| date_label(at, offset_minutes)).unwrap_or_default(),
            )
        }
        (PropertyType::Checkbox, true) => {
            let on = !matches!(value, Some(CellValue::Checkbox(false)));
            (
                FilterInput::Checkbox(on),
                if on { "ticked" } else { "not ticked" }.to_owned(),
            )
        }
    };
    FilterChip {
        column: column.id,
        name: column.name.clone(),
        kind: column.kind.clone(),
        op: filter.op.clone(),
        op_label: op_label(&filter.op, &column.kind),
        value_label,
        ops: filter_ops(&column.kind),
        input,
    }
}

/// A sort as it reads in the bar: the direction in words fit for its type.
#[must_use]
pub fn direction_label(kind: &PropertyType, descending: bool) -> &'static str {
    match (kind, descending) {
        (PropertyType::Number, false) => "1 → 9",
        (PropertyType::Number, true) => "9 → 1",
        (PropertyType::Date, false) => "Old → New",
        (PropertyType::Date, true) => "New → Old",
        (PropertyType::Checkbox, false) => "Unticked first",
        (PropertyType::Checkbox, true) => "Ticked first",
        (PropertyType::Select | PropertyType::MultiSelect, false) => "Option order",
        (PropertyType::Select | PropertyType::MultiSelect, true) => "Reverse order",
        (_, false) => "A → Z",
        (_, true) => "Z → A",
    }
}

#[cfg(test)]
mod tests;
