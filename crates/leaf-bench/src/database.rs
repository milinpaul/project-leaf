//! A 10,000-row database for the database budgets (E09-S07, E09-S10). Rows have a name, a price, a status (five
//! options) and a done box, spread so a filter keeps some of them. It is built once and saved as bytes, so each bench
//! opens it as the app does: from its saved document, a first screen of rows read, the rest later.

use leaf_core::{ColumnId, OptionId, PageId};
use leaf_editor::{DatabaseEditor, FIRST_SCREEN_ROWS};
use leaf_model::{CellValue, Database, DatabaseCommand, DatabaseOutcome, FilterOp, PropertyType};

use crate::BenchError;

/// Rows in the database the database benches use.
pub const DATABASE_ROWS: usize = 10_000;

/// A saved database and what the benches need to change it.
pub struct SavedDatabase {
    /// The saved document.
    pub bytes: Vec<u8>,
    /// Its rows, in order.
    pub rows: Vec<PageId>,
    /// The text column.
    pub name: ColumnId,
    /// The number column.
    pub price: ColumnId,
    /// The select column.
    pub status: ColumnId,
    /// The status options.
    pub options: Vec<OptionId>,
}

impl SavedDatabase {
    /// Opens it as the app does: the saved document loaded and its first screen of rows read.
    pub fn open(&self) -> Result<DatabaseEditor, BenchError> {
        let mut editor = DatabaseEditor::new(Database::from_bytes(&self.bytes)?);
        editor.read_more(FIRST_SCREEN_ROWS)?;
        Ok(editor)
    }

    /// Opens it with every row read, sorted by price and filtered to the third status, as a table the person
    /// already has open; returns the editor.
    pub fn sorted_filtered(&self) -> Result<DatabaseEditor, BenchError> {
        let mut editor = self.open()?;
        editor.read_more(usize::MAX)?;
        editor.add_sort(self.price)?;
        let filter = editor.add_filter(self.status)?;
        let third = self
            .options
            .get(2)
            .copied()
            .ok_or(BenchError::Setup("no third status"))?;
        editor.set_filter_value(filter, Some(CellValue::Option(third)))?;
        Ok(editor)
    }
}

/// Builds and saves a database of `rows` rows.
pub fn saved_database(rows: usize) -> Result<SavedDatabase, BenchError> {
    let mut db = Database::new();
    let column = |db: &mut Database, name: &str, kind: PropertyType| -> Result<ColumnId, BenchError> {
        let index = db.column_ids().len();
        match db.apply(DatabaseCommand::AddColumn {
            index,
            name: name.into(),
            kind,
        })? {
            DatabaseOutcome::ColumnCreated(id) => Ok(id),
            _ => Err(BenchError::Setup("a column wasn't created")),
        }
    };
    let name = column(&mut db, "Name", PropertyType::Text)?;
    let price = column(&mut db, "Price", PropertyType::Number)?;
    let status = column(&mut db, "Status", PropertyType::Select)?;
    let done = column(&mut db, "Done", PropertyType::Checkbox)?;
    let mut options = Vec::new();
    for option in ["Backlog", "To do", "Doing", "Review", "Done"] {
        match db.apply(DatabaseCommand::AddOption {
            column: status,
            name: option.into(),
            color: "gray".into(),
        })? {
            DatabaseOutcome::OptionCreated(id) => options.push(id),
            _ => return Err(BenchError::Setup("an option wasn't created")),
        }
    }
    let mut ids = Vec::with_capacity(rows);
    for i in 0..rows {
        let row = PageId::new();
        db.apply(DatabaseCommand::AddRow { row, index: i })?;
        let spread = (i as u64).wrapping_mul(2_654_435_761) % 100_000;
        let picked = options
            .get(i % options.len())
            .copied()
            .ok_or(BenchError::Setup("no options"))?;
        let cells = [
            (name, CellValue::Text(format!("Item {spread}"))),
            #[expect(clippy::cast_precision_loss, reason = "small bench numbers")]
            (price, CellValue::Number(spread as f64 / 100.0)),
            (status, CellValue::Option(picked)),
            (done, CellValue::Checkbox(i % 3 == 0)),
        ];
        for (column, value) in cells {
            db.apply(DatabaseCommand::SetCell {
                row,
                column,
                value: Some(value),
            })?;
        }
        ids.push(row);
    }
    Ok(SavedDatabase {
        bytes: db.to_bytes()?,
        rows: ids,
        name,
        price,
        status,
        options,
    })
}

/// A database editor over `rows` rows, every row read, sorted by price and filtered to one status, with a typed
/// "contains" filter on the name after them (filter 1), plus the name column (E09-S07).
pub fn sorted_filtered_database(rows: usize) -> Result<(DatabaseEditor, ColumnId), BenchError> {
    let saved = saved_database(rows)?;
    let mut editor = saved.sorted_filtered()?;
    let typed = editor.add_filter(saved.name)?;
    editor.set_filter_op(typed, FilterOp::Contains)?;
    Ok((editor, saved.name))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]

    use leaf_editor::FIRST_SCREEN_ROWS;

    use super::{saved_database, sorted_filtered_database};

    #[test]
    fn the_database_is_read_sorted_and_filtered_to_one_status_in_five() {
        let (mut editor, _) = sorted_filtered_database(500).unwrap();
        let view = editor.view(|_| String::new(), 0).unwrap();
        assert!(view.complete);
        assert_eq!(view.total_rows, 500);
        assert_eq!(view.rows.len(), 100, "one status in five");
        let bar = editor.query_bar(0).unwrap();
        assert_eq!((bar.sorts.len(), bar.filters.len()), (1, 2));
        assert!(editor.type_filter_text(1, "Item 1").unwrap());
        assert!(
            editor.view(|_| String::new(), 0).unwrap().rows.len() < 100,
            "the typed filter narrows"
        );
    }

    #[test]
    fn opening_reads_the_first_screen_and_the_rest_reads_on() {
        let saved = saved_database(500).unwrap();
        assert_eq!(saved.rows.len(), 500);
        let mut editor = saved.open().unwrap();
        assert_eq!(editor.rows_read(), FIRST_SCREEN_ROWS);
        let view = editor.view(|_| String::new(), 0).unwrap();
        assert!(!view.complete);
        assert!(editor.read_more(usize::MAX).unwrap());
        assert_eq!(editor.rows_read(), 500);
    }
}
