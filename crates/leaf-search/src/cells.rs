//! A database's text, for search (E09-S10): each row's text and URL cells, indexed under the row's page as
//! `property` entries. A row is a page, so a hit opens the row, whose properties sit under its title.
//!
//! Cells are read from the database document, which changes independently of the row pages, so the index keeps
//! each database's version and, when it changes, writes only the cells whose text changed
//! ([`crate::SearchIndex::index_database`]).

use leaf_core::{ColumnId, PageId};
use leaf_model::{CellValue, Database, PropertyType};

use crate::SearchError;

/// The kind recorded for a cell's entry. Such entries have no block: their `block_id` is
/// `<database>/<column>`, which never reads as a block.
pub const PROPERTY_KIND: &str = "property";

/// The text of one cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellText {
    /// The row's page.
    pub page: PageId,
    /// The column.
    pub column: ColumnId,
    /// What the cell holds, as typed.
    pub text: String,
}

/// The text and URL cells of `database`'s rows that search should find: those whose row page is `wanted` (not in
/// the trash). Empty cells are left out. In row order.
pub fn database_text(
    database: &Database,
    wanted: &dyn Fn(PageId) -> bool,
) -> Result<Vec<CellText>, SearchError> {
    let columns: Vec<ColumnId> = database
        .columns()?
        .into_iter()
        .filter(|c| matches!(c.kind, PropertyType::Text | PropertyType::Url))
        .map(|c| c.id)
        .collect();
    let mut out = Vec::new();
    if columns.is_empty() {
        return Ok(out);
    }
    for page in database.rows().into_iter().filter(|&row| wanted(row)) {
        for (column, value) in database.row_values(page)? {
            if let CellValue::Text(text) = value
                && columns.contains(&column)
                && !text.trim().is_empty()
            {
                out.push(CellText { page, column, text });
            }
        }
    }
    Ok(out)
}
