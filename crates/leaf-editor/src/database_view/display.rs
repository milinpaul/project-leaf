//! What a cell shows, converted for its column's current type (ADR-066 answer 3).
//!
//! The model keeps each value as it was stored, so after a column's type changes a cell may hold a value of the
//! old type. Here it is converted where it can be ("12" shows as the number 12 in a number column, 12 as "12" in a
//! text column) and shows empty where it can't; the stored value stays until the cell is edited. The same
//! conversions give the values that sorting and filtering compare.

use leaf_core::OptionId;
use leaf_model::{CellValue, Column, PropertyType, SelectOption};

use crate::edited::{MONTHS, civil_from_days};

/// One option as a cell shows it: a coloured chip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// The option.
    pub option: OptionId,
    /// Its name.
    pub name: String,
    /// Its colour name (the UI maps names it doesn't know to a default).
    pub color: String,
}

/// What one cell of the table shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellDisplay {
    /// Nothing.
    Empty,
    /// Text.
    Text(String),
    /// A number, written out.
    Number(String),
    /// A date, written out ("30 Oct 2026").
    Date(String),
    /// A checkbox, ticked or not (never empty).
    Checkbox(bool),
    /// A web address.
    Url(String),
    /// Select and multi-select options, in the column's order.
    Options(Vec<Chip>),
    /// A value of a column type from a newer version: shown as such, read-only.
    Unknown,
}

const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 86_400_000;

/// A date as the table shows it, in the device's time zone (`offset_minutes` east of UTC).
#[must_use]
pub fn date_label(at: i64, offset_minutes: i32) -> String {
    let local = at + i64::from(offset_minutes) * MINUTE_MS;
    let (year, month, day) = civil_from_days(local.div_euclid(DAY_MS));
    let month = MONTHS
        .get(usize::try_from(month - 1).unwrap_or(0))
        .copied()
        .unwrap_or("Jan");
    format!("{day} {month} {year}")
}

/// A number written out: whole numbers without a decimal point.
fn number_label(n: f64) -> String {
    format!("{n}")
}

/// The value as a number, if it reads as one.
pub(super) fn as_number(value: &CellValue) -> Option<f64> {
    match value {
        CellValue::Number(n) => Some(*n),
        CellValue::Text(text) => text.trim().parse::<f64>().ok().filter(|n| n.is_finite()),
        _ => None,
    }
}

/// The value as text, as a text column would show it.
pub(super) fn as_text(value: &CellValue, column: &Column, offset_minutes: i32) -> Option<String> {
    let text = match value {
        // An option's ID from when the column was a select reads as the option's name.
        CellValue::Text(text) => match text.parse::<OptionId>() {
            Ok(id) => column
                .options
                .iter()
                .find(|o| o.id == id)
                .map_or_else(|| text.clone(), |o| o.name.clone()),
            Err(_) => text.clone(),
        },
        CellValue::Number(n) => number_label(*n),
        CellValue::Date(at) => date_label(*at, offset_minutes),
        CellValue::Checkbox(on) => if *on { "Yes" } else { "No" }.to_owned(),
        CellValue::Option(_) | CellValue::Options(_) => options(value, column)
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        CellValue::Unknown => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// The column's options the value picks, in the column's order. Options since deleted are left out.
pub(super) fn options<'a>(value: &CellValue, column: &'a Column) -> Vec<&'a SelectOption> {
    let picked: Vec<OptionId> = match value {
        CellValue::Option(id) => vec![*id],
        CellValue::Options(ids) => ids.iter().copied().collect(),
        CellValue::Text(text) => text.parse().ok().into_iter().collect(),
        _ => Vec::new(),
    };
    column.options.iter().filter(|o| picked.contains(&o.id)).collect()
}

/// What `value` (or an empty cell) shows in `column`.
#[must_use]
pub fn display(value: Option<&CellValue>, column: &Column, offset_minutes: i32) -> CellDisplay {
    if let PropertyType::Unknown(_) = column.kind {
        return if value.is_some() {
            CellDisplay::Unknown
        } else {
            CellDisplay::Empty
        };
    }
    if column.kind == PropertyType::Checkbox {
        return CellDisplay::Checkbox(matches!(value, Some(CellValue::Checkbox(true))));
    }
    let Some(value) = value else {
        return CellDisplay::Empty;
    };
    let shown = match column.kind {
        PropertyType::Text => as_text(value, column, offset_minutes).map(CellDisplay::Text),
        PropertyType::Url => as_text(value, column, offset_minutes).map(CellDisplay::Url),
        PropertyType::Number => as_number(value).map(|n| CellDisplay::Number(number_label(n))),
        PropertyType::Date => match value {
            CellValue::Date(at) => Some(CellDisplay::Date(date_label(*at, offset_minutes))),
            _ => None,
        },
        PropertyType::Select | PropertyType::MultiSelect => {
            let mut chips: Vec<Chip> = options(value, column)
                .into_iter()
                .map(|o| Chip {
                    option: o.id,
                    name: o.name.clone(),
                    color: o.color.clone(),
                })
                .collect();
            if column.kind == PropertyType::Select {
                chips.truncate(1);
            }
            (!chips.is_empty()).then_some(CellDisplay::Options(chips))
        }
        PropertyType::Checkbox | PropertyType::Unknown(_) => None,
    };
    shown.unwrap_or(CellDisplay::Empty)
}
