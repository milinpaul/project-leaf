//! Which rows the table shows and in what order: the view's filters, then its sort (E09-S04).
//!
//! Values are compared as their column's current type shows them (see `display`), so a value left over from an
//! earlier type sorts and filters the way it reads. Sorting is stable, so rows that tie keep the database's own
//! order, and empty cells always go last, whichever way the sort runs. A sort or filter on a column that no longer
//! exists, or a filter condition from a newer version, is ignored.

use std::cmp::Ordering;
use std::collections::HashMap;

use leaf_core::{ColumnId, PageId};
use leaf_model::{CellValue, Column, Filter, FilterOp, PropertyType, Sort};

use super::display::{as_number, as_text, options};

/// A value reduced to what comparing needs.
#[derive(Debug, Clone, PartialEq)]
enum Key {
    Text(String),
    Number(f64),
    Date(i64),
    Checkbox(bool),
    /// Positions of the picked options in the column's list, in that order.
    Options(Vec<usize>),
}

/// The comparable form of `value` in `column`, or `None` when the cell reads as empty.
fn key(value: Option<&CellValue>, column: &Column) -> Option<Key> {
    if column.kind == PropertyType::Checkbox {
        return Some(Key::Checkbox(matches!(value, Some(CellValue::Checkbox(true)))));
    }
    let value = value?;
    match column.kind {
        PropertyType::Text | PropertyType::Url => {
            as_text(value, column, 0).map(|t| Key::Text(t.to_lowercase()))
        }
        PropertyType::Number => as_number(value).map(Key::Number),
        PropertyType::Date => match value {
            CellValue::Date(at) => Some(Key::Date(*at)),
            _ => None,
        },
        PropertyType::Select | PropertyType::MultiSelect => {
            let picked = options(value, column);
            let positions: Vec<usize> = column
                .options
                .iter()
                .enumerate()
                .filter(|(_, o)| picked.iter().any(|p| p.id == o.id))
                .map(|(i, _)| i)
                .collect();
            (!positions.is_empty()).then_some(Key::Options(positions))
        }
        PropertyType::Checkbox | PropertyType::Unknown(_) => None,
    }
}

fn compare(a: &Key, b: &Key) -> Ordering {
    match (a, b) {
        (Key::Text(a), Key::Text(b)) => a.cmp(b),
        (Key::Number(a), Key::Number(b)) => a.total_cmp(b),
        (Key::Date(a), Key::Date(b)) => a.cmp(b),
        (Key::Checkbox(a), Key::Checkbox(b)) => a.cmp(b),
        (Key::Options(a), Key::Options(b)) => a.cmp(b),
        // One column gives one kind of key; anything else is equal rather than a guess.
        _ => Ordering::Equal,
    }
}

/// One row's cached values, by column.
pub(super) type Values = HashMap<ColumnId, CellValue>;

/// Whether the row passes every filter.
pub(super) fn passes(values: &Values, filters: &[Filter], columns: &[Column]) -> bool {
    filters.iter().all(|filter| {
        let Some(column) = columns.iter().find(|c| c.id == filter.column) else {
            return true;
        };
        let cell = key(values.get(&column.id), column);
        let wanted = filter.value.as_ref().and_then(|v| key(Some(v), column));
        match &filter.op {
            FilterOp::IsEmpty => is_empty(cell.as_ref()),
            FilterOp::IsNotEmpty => !is_empty(cell.as_ref()),
            FilterOp::Unknown(_) => true,
            op => match wanted {
                // A comparison with nothing to compare against doesn't narrow anything yet.
                None => true,
                Some(wanted) => matches(op, cell.as_ref(), &wanted),
            },
        }
    })
}

/// An unticked checkbox counts as empty, as it shows nothing.
fn is_empty(cell: Option<&Key>) -> bool {
    matches!(cell, None | Some(Key::Checkbox(false)))
}

fn matches(op: &FilterOp, cell: Option<&Key>, wanted: &Key) -> bool {
    let contains = || match (cell, wanted) {
        (Some(Key::Text(text)), Key::Text(part)) => text.contains(part.as_str()),
        (Some(Key::Options(have)), Key::Options(want)) => want.iter().all(|w| have.contains(w)),
        (Some(cell), wanted) => cell == wanted,
        (None, _) => false,
    };
    let order = || cell.map(|cell| compare(cell, wanted));
    match op {
        FilterOp::Is => match (cell, wanted) {
            (Some(Key::Options(_)), Key::Options(_)) => contains(),
            _ => cell == Some(wanted),
        },
        FilterOp::IsNot => match (cell, wanted) {
            (Some(Key::Options(_)), Key::Options(_)) => !contains(),
            _ => cell != Some(wanted),
        },
        FilterOp::Contains => contains(),
        FilterOp::DoesNotContain => !contains(),
        FilterOp::Greater => order() == Some(Ordering::Greater),
        FilterOp::Less => order() == Some(Ordering::Less),
        FilterOp::IsEmpty | FilterOp::IsNotEmpty | FilterOp::Unknown(_) => true,
    }
}

/// Sorts `rows` (already in the database's order) by the view's sort, stably.
pub(super) fn sort(
    rows: &mut [PageId],
    sorts: &[Sort],
    columns: &[Column],
    values: &HashMap<PageId, Values>,
) {
    let keys: Vec<(&Column, bool)> = sorts
        .iter()
        .filter_map(|s| {
            columns
                .iter()
                .find(|c| c.id == s.column)
                .map(|c| (c, s.descending))
        })
        .collect();
    if keys.is_empty() {
        return;
    }
    let empty = Values::new();
    rows.sort_by(|a, b| {
        let (a, b) = (values.get(a).unwrap_or(&empty), values.get(b).unwrap_or(&empty));
        for &(column, descending) in &keys {
            let ordering = match (key(a.get(&column.id), column), key(b.get(&column.id), column)) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) if descending => compare(&b, &a),
                (Some(a), Some(b)) => compare(&a, &b),
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    });
}
