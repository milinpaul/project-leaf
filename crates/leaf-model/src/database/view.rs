//! A database's table view settings: how rows are sorted and which are shown (ADR-066, E09-S07 draws them).
//!
//! The sort and the filters are each one plain value in the `"view"` map, so a change replaces the whole list
//! and two devices changing them at once settle on one list rather than a mix. What a newer version of Leaf
//! stores in a sort or filter (an extra key, a filter condition this version doesn't know) is kept as it was
//! and written back with it.

use std::collections::BTreeMap;

use leaf_core::ColumnId;
use loro::LoroValue;

use super::property::PropertyType;
use crate::database::CellValue;

/// Keys of the `"view"` map and of each sort or filter in it.
pub(crate) mod keys {
    pub const SORT: &str = "sort";
    pub const FILTERS: &str = "filters";
    pub const COLUMN: &str = "column";
    pub const DESCENDING: &str = "descending";
    pub const OP: &str = "op";
    pub const VALUE: &str = "value";
}

/// What a newer version stored in a sort or filter beyond what this version reads. Opaque, carried along.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extra(BTreeMap<String, LoroValue>);

/// One sort key: rows are ordered by the first, ties by the next, and so on.
#[derive(Debug, Clone, PartialEq)]
pub struct Sort {
    /// The column to sort by.
    pub column: ColumnId,
    /// Largest, latest or Z first.
    pub descending: bool,
    extra: Extra,
}

impl Sort {
    /// Sorts by `column`.
    #[must_use]
    pub fn new(column: ColumnId, descending: bool) -> Self {
        Self {
            column,
            descending,
            extra: Extra::default(),
        }
    }
}

/// How a filter compares a cell. Which ones make sense depends on the column's type; the view decides that
/// (E09-S07), the model only stores them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOp {
    /// Equal to the value.
    Is,
    /// Not equal to the value.
    IsNot,
    /// Text contains it, or the options include it.
    Contains,
    /// Text doesn't contain it, or the options don't include it.
    DoesNotContain,
    /// The cell is empty.
    IsEmpty,
    /// The cell is not empty.
    IsNotEmpty,
    /// A number above, or a date after, the value.
    Greater,
    /// A number below, or a date before, the value.
    Less,
    /// A condition from a newer version, by name. Kept; the view ignores it.
    Unknown(String),
}

impl FilterOp {
    fn name(&self) -> &str {
        match self {
            Self::Is => "is",
            Self::IsNot => "is_not",
            Self::Contains => "contains",
            Self::DoesNotContain => "does_not_contain",
            Self::IsEmpty => "is_empty",
            Self::IsNotEmpty => "is_not_empty",
            Self::Greater => "greater",
            Self::Less => "less",
            Self::Unknown(name) => name,
        }
    }

    fn from_name(name: &str) -> Self {
        match name {
            "is" => Self::Is,
            "is_not" => Self::IsNot,
            "contains" => Self::Contains,
            "does_not_contain" => Self::DoesNotContain,
            "is_empty" => Self::IsEmpty,
            "is_not_empty" => Self::IsNotEmpty,
            "greater" => Self::Greater,
            "less" => Self::Less,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

/// One filter: a row shows only if every filter matches it.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    /// The column it looks at.
    pub column: ColumnId,
    /// How it compares.
    pub op: FilterOp,
    /// What it compares with (`None` for "is empty" and "is not empty").
    pub value: Option<CellValue>,
    extra: Extra,
}

impl Filter {
    /// A filter on `column`.
    #[must_use]
    pub fn new(column: ColumnId, op: FilterOp, value: Option<CellValue>) -> Self {
        Self {
            column,
            op,
            value,
            extra: Extra::default(),
        }
    }
}

/// Reads the list stored under a view key; entries without a readable column are skipped (and so dropped on the
/// next write, which only a broken document has).
fn entries(value: Option<LoroValue>) -> Vec<BTreeMap<String, LoroValue>> {
    match value {
        Some(LoroValue::List(list)) => list
            .iter()
            .filter_map(|entry| match entry {
                LoroValue::Map(map) => Some(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn column_of(entry: &mut BTreeMap<String, LoroValue>) -> Option<ColumnId> {
    match entry.remove(keys::COLUMN) {
        Some(LoroValue::String(text)) => text.parse().ok(),
        _ => None,
    }
}

pub(crate) fn read_sort(value: Option<LoroValue>) -> Vec<Sort> {
    entries(value)
        .into_iter()
        .filter_map(|mut entry| {
            let column = column_of(&mut entry)?;
            let descending = matches!(entry.remove(keys::DESCENDING), Some(LoroValue::Bool(true)));
            Some(Sort {
                column,
                descending,
                extra: Extra(entry),
            })
        })
        .collect()
}

pub(crate) fn write_sort(sort: &[Sort]) -> LoroValue {
    LoroValue::from(
        sort.iter()
            .map(|s| {
                let mut map = s.extra.0.clone();
                map.insert(keys::COLUMN.into(), s.column.to_string().into());
                map.insert(keys::DESCENDING.into(), s.descending.into());
                LoroValue::from(map.into_iter().collect::<std::collections::HashMap<_, _>>())
            })
            .collect::<Vec<_>>(),
    )
}

/// Reads the filters. A filter's value is read without its column's type, so an option ID reads as text;
/// `kind_of` gives the column's type where it is known.
pub(crate) fn read_filters(
    value: Option<LoroValue>,
    kind_of: impl Fn(ColumnId) -> Option<PropertyType>,
) -> Vec<Filter> {
    entries(value)
        .into_iter()
        .filter_map(|mut entry| {
            let column = column_of(&mut entry)?;
            let op = match entry.remove(keys::OP) {
                Some(LoroValue::String(name)) => FilterOp::from_name(&name),
                _ => return None,
            };
            if let FilterOp::Unknown(name) = &op {
                // Kept whole, value and all, to write back exactly.
                entry.insert(keys::OP.into(), name.as_str().into());
                return Some(Filter {
                    column,
                    op,
                    value: None,
                    extra: Extra(entry),
                });
            }
            let kind = kind_of(column).unwrap_or(PropertyType::Text);
            let value = match entry.remove(keys::VALUE) {
                Some(LoroValue::List(ids)) => Some(CellValue::Options(
                    ids.iter()
                        .filter_map(|id| match id {
                            LoroValue::String(text) => text.parse().ok(),
                            _ => None,
                        })
                        .collect(),
                )),
                Some(LoroValue::Null) | None => None,
                Some(other) => Some(CellValue::from_loro(&other, &kind)),
            };
            Some(Filter {
                column,
                op,
                value,
                extra: Extra(entry),
            })
        })
        .collect()
}

pub(crate) fn write_filters(filters: &[Filter]) -> LoroValue {
    LoroValue::from(
        filters
            .iter()
            .map(|f| {
                let mut map = f.extra.0.clone();
                map.insert(keys::COLUMN.into(), f.column.to_string().into());
                if !matches!(f.op, FilterOp::Unknown(_)) {
                    map.insert(keys::OP.into(), f.op.name().into());
                    let value = match &f.value {
                        Some(CellValue::Options(ids)) => {
                            LoroValue::from(ids.iter().map(ToString::to_string).collect::<Vec<_>>())
                        }
                        Some(value) => value.to_loro().unwrap_or(LoroValue::Null),
                        None => LoroValue::Null,
                    };
                    map.insert(keys::VALUE.into(), value);
                }
                LoroValue::from(map.into_iter().collect::<std::collections::HashMap<_, _>>())
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests;
