//! What a database column is and what its cells hold (ADR 0066).
//!
//! A column's type decides which values its cells take. Cells are stored as plain Loro values (last writer wins
//! per cell), except multi-select, which is a mergeable map of option ID to `true` so tags added on two devices
//! are all kept. Reading goes by what is stored, so a value written under an earlier type (before a retype, or
//! by another device at the same time) still reads back as itself: the editor converts it for display (ADR 0066
//! answer 3), and the stored value stays until the cell is edited.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId};
use loro::{Container, LoroMap, LoroMovableList, LoroValue, ValueOrContainer};

use crate::ModelError;
use crate::page_meta::value;

/// Keys of a column's schema map and of an option's map.
pub(crate) mod keys {
    pub const NAME: &str = "name";
    pub const TYPE: &str = "type";
    pub const HIDDEN: &str = "hidden";
    pub const WIDTH: &str = "width";
    pub const OPTIONS: &str = "options";
    pub const ID: &str = "id";
    pub const COLOR: &str = "color";
}

/// A column's type. Types this version doesn't know (from a newer Leaf) read as `Unknown` and keep their name;
/// such columns are read-only here, so nothing that newer version stored is lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyType {
    /// Plain text (ADR 0066 answer 2: no formatting).
    Text,
    /// A number.
    Number,
    /// One option out of the column's list.
    Select,
    /// Any number of options out of the column's list.
    MultiSelect,
    /// A point in time, unix milliseconds.
    Date,
    /// Ticked or not.
    Checkbox,
    /// A web address, as text.
    Url,
    /// A type from a newer version, by its stored name.
    Unknown(String),
}

impl PropertyType {
    /// The name stored in the column's schema, and shown for unknown types.
    #[must_use]
    pub fn type_name(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Number => "number",
            Self::Select => "select",
            Self::MultiSelect => "multi_select",
            Self::Date => "date",
            Self::Checkbox => "checkbox",
            Self::Url => "url",
            Self::Unknown(name) => name,
        }
    }

    /// The type stored under `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "text" => Self::Text,
            "number" => Self::Number,
            "select" => Self::Select,
            "multi_select" => Self::MultiSelect,
            "date" => Self::Date,
            "checkbox" => Self::Checkbox,
            "url" => Self::Url,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// Whether the column has a list of options to pick from.
    #[must_use]
    pub fn has_options(&self) -> bool {
        matches!(self, Self::Select | Self::MultiSelect)
    }
}

/// One option of a select or multi-select column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    /// Its permanent ID, which cells hold.
    pub id: OptionId,
    /// What it reads as.
    pub name: String,
    /// A colour name from the theme ("gray", "green", ...). Names this version doesn't know are kept and drawn
    /// in a default colour.
    pub color: String,
}

/// A column as the database describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Its permanent ID.
    pub id: ColumnId,
    /// Its name, shown in the header.
    pub name: String,
    /// Its type.
    pub kind: PropertyType,
    /// Whether the table view leaves it out.
    pub hidden: bool,
    /// Its width in the table view, or `None` for the default.
    pub width: Option<f64>,
    /// Its options, in order (select and multi-select columns; empty otherwise).
    pub options: Vec<SelectOption>,
}

/// What a cell holds, read by what is stored.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// Text, or a web address.
    Text(String),
    /// A number. Never NaN or infinite.
    Number(f64),
    /// A point in time, unix milliseconds.
    Date(i64),
    /// A checkbox.
    Checkbox(bool),
    /// A select column's option.
    Option(OptionId),
    /// A multi-select column's options.
    Options(BTreeSet<OptionId>),
    /// Something this version can't read (from a newer Leaf). Kept as it is.
    Unknown,
}

impl CellValue {
    /// Whether this value is one `kind` takes. Options must also be the column's, which
    /// [`check_options`] checks.
    pub(crate) fn fits(&self, kind: &PropertyType) -> bool {
        matches!(
            (kind, self),
            (PropertyType::Text | PropertyType::Url, Self::Text(_))
                | (PropertyType::Date, Self::Date(_))
                | (PropertyType::Checkbox, Self::Checkbox(_))
                | (PropertyType::Select, Self::Option(_))
                | (PropertyType::MultiSelect, Self::Options(_))
        ) || matches!((kind, self), (PropertyType::Number, Self::Number(n)) if n.is_finite())
    }

    /// The value as one plain Loro value, for every kind but `Options` (a container of its own) and `Unknown`
    /// (never written).
    pub(crate) fn to_loro(&self) -> Option<LoroValue> {
        Some(match self {
            Self::Text(text) => text.as_str().into(),
            Self::Number(n) => (*n).into(),
            Self::Date(at) => (*at).into(),
            Self::Checkbox(on) => (*on).into(),
            Self::Option(id) => id.to_string().into(),
            Self::Options(_) | Self::Unknown => return None,
        })
    }

    /// Reads a stored plain value. Text that is an option ID of a select column reads as that option.
    pub(crate) fn from_loro(value: &LoroValue, kind: &PropertyType) -> Self {
        match value {
            LoroValue::String(text) => match (kind, text.parse::<OptionId>()) {
                (PropertyType::Select, Ok(id)) => Self::Option(id),
                _ => Self::Text(text.to_string()),
            },
            LoroValue::Double(n) => Self::Number(*n),
            LoroValue::I64(at) => Self::Date(*at),
            LoroValue::Bool(on) => Self::Checkbox(*on),
            LoroValue::Map(map) => Self::Options(
                map.iter()
                    .filter(|(_, on)| matches!(on, LoroValue::Bool(true)))
                    .filter_map(|(id, _)| id.parse().ok())
                    .collect(),
            ),
            _ => Self::Unknown,
        }
    }

    /// Reads a cell as Loro holds it in a row's map.
    pub(crate) fn read(cell: ValueOrContainer, kind: &PropertyType) -> Option<Self> {
        match cell {
            ValueOrContainer::Value(LoroValue::Null) => None,
            ValueOrContainer::Value(value) => Some(Self::from_loro(&value, kind)),
            ValueOrContainer::Container(Container::Map(map)) => {
                Some(Self::from_loro(&map.get_deep_value(), kind))
            }
            ValueOrContainer::Container(_) => Some(Self::Unknown),
        }
    }
}

/// Fails unless every option in `value` is one of `column`'s.
pub(crate) fn check_options(column: &Column, value: &CellValue) -> Result<(), ModelError> {
    let picked: Vec<OptionId> = match value {
        CellValue::Option(id) => vec![*id],
        CellValue::Options(ids) => ids.iter().copied().collect(),
        _ => return Ok(()),
    };
    for option in picked {
        if !column.options.iter().any(|o| o.id == option) {
            return Err(ModelError::OptionNotFound {
                column: column.id,
                option,
            });
        }
    }
    Ok(())
}

/// Sets a multi-select cell's map to exactly `options`, touching only the options that change, so tags added
/// on another device at the same time are kept and options this version can't read are left alone.
pub(crate) fn write_options(map: &LoroMap, options: &BTreeSet<OptionId>) -> Result<(), ModelError> {
    let mut stored = Vec::new();
    map.for_each(|key, _| stored.push(key.to_owned()));
    for key in stored {
        if let Ok(id) = key.parse::<OptionId>()
            && !options.contains(&id)
        {
            map.delete(&key)?;
        }
    }
    for id in options {
        let key = id.to_string();
        if !matches!(
            map.get(&key),
            Some(ValueOrContainer::Value(LoroValue::Bool(true)))
        ) {
            map.insert(&key, true)?;
        }
    }
    Ok(())
}

/// A column from its schema map. Missing or mistyped fields read as their defaults; a missing type reads as
/// an unknown one, so the column is read-only rather than guessed at.
pub(super) fn read_column(id: ColumnId, schema: &LoroMap) -> Column {
    let text = |key| match value(schema, key) {
        Some(LoroValue::String(s)) => s.to_string(),
        _ => String::new(),
    };
    let kind = PropertyType::from_name(&text(keys::TYPE));
    let options = match schema.get(keys::OPTIONS) {
        Some(ValueOrContainer::Container(Container::MovableList(list))) => read_options(&list),
        _ => Vec::new(),
    };
    Column {
        id,
        name: text(keys::NAME),
        kind,
        hidden: matches!(value(schema, keys::HIDDEN), Some(LoroValue::Bool(true))),
        width: match value(schema, keys::WIDTH) {
            Some(LoroValue::Double(w)) => Some(w),
            _ => None,
        },
        options,
    }
}

/// A column's options in order. Options without a readable ID are skipped; the same ID twice (two devices'
/// lists merged) reads once.
fn read_options(list: &LoroMovableList) -> Vec<SelectOption> {
    let mut out: Vec<SelectOption> = Vec::new();
    for item in 0..list.len() {
        let Some(ValueOrContainer::Container(Container::Map(map))) = list.get(item) else {
            continue;
        };
        let text = |key| match value(&map, key) {
            Some(LoroValue::String(s)) => s.to_string(),
            _ => String::new(),
        };
        let Ok(id) = text(keys::ID).parse::<OptionId>() else {
            continue;
        };
        if out.iter().any(|o| o.id == id) {
            continue;
        }
        out.push(SelectOption {
            id,
            name: text(keys::NAME),
            color: text(keys::COLOR),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CellValue, PropertyType};

    #[test]
    fn type_names_round_trip_and_unknown_ones_keep_their_name() {
        for kind in [
            PropertyType::Text,
            PropertyType::Number,
            PropertyType::Select,
            PropertyType::MultiSelect,
            PropertyType::Date,
            PropertyType::Checkbox,
            PropertyType::Url,
            PropertyType::Unknown("formula".into()),
        ] {
            assert_eq!(PropertyType::from_name(kind.type_name()), kind);
        }
        assert!(PropertyType::Select.has_options() && PropertyType::MultiSelect.has_options());
        assert!(!PropertyType::Text.has_options());
    }

    #[test]
    fn values_fit_only_their_types_and_numbers_must_be_finite() {
        assert!(CellValue::Text("a".into()).fits(&PropertyType::Url));
        assert!(CellValue::Number(1.5).fits(&PropertyType::Number));
        assert!(!CellValue::Number(f64::NAN).fits(&PropertyType::Number));
        assert!(!CellValue::Number(f64::INFINITY).fits(&PropertyType::Number));
        assert!(!CellValue::Checkbox(true).fits(&PropertyType::Text));
        assert!(!CellValue::Text("a".into()).fits(&PropertyType::Unknown("x".into())));
        assert!(CellValue::Unknown.to_loro().is_none());
    }
}
