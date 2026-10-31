//! A 10,000-row database, read in, for the sort and filter benches (E09-S07). Rows have a name, a price, a status
//! (five options) and a done box, spread so a filter keeps some of them.

use leaf_core::{ColumnId, PageId};
use leaf_editor::DatabaseEditor;
use leaf_model::{CellValue, Database, DatabaseCommand, DatabaseOutcome, FilterOp, PropertyType};

use crate::BenchError;

/// Rows in the database the sort and filter benches use.
pub const DATABASE_ROWS: usize = 10_000;

/// A database editor over `rows` rows, every row read, sorted by price and filtered to one status, plus the column
/// a typed filter goes on (the name).
pub fn sorted_filtered_database(rows: usize) -> Result<(DatabaseEditor, ColumnId), BenchError> {
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
    for i in 0..rows {
        let row = PageId::new();
        db.apply(DatabaseCommand::AddRow { row, index: i })?;
        let spread = (i as u64).wrapping_mul(2_654_435_761) % 100_000;
        let cells = [
            (name, CellValue::Text(format!("Item {spread}"))),
            #[expect(clippy::cast_precision_loss, reason = "small bench numbers")]
            (price, CellValue::Number(spread as f64 / 100.0)),
            (status, CellValue::Option(options[i % options.len()])),
            (done, CellValue::Checkbox(i % 3 == 0)),
        ];
        for (column, value) in cells {
            db.apply(DatabaseCommand::SetCell {
                row,
                column,
                value: Some(value),
            })?;
        }
    }
    let mut editor = DatabaseEditor::new(db);
    editor.read_more(usize::MAX)?;
    editor.add_sort(price)?;
    let filter = editor.add_filter(status)?;
    editor.set_filter_value(filter, Some(CellValue::Option(options[2])))?;
    let typed = editor.add_filter(name)?;
    editor.set_filter_op(typed, FilterOp::Contains)?;
    Ok((editor, name))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]

    use super::sorted_filtered_database;

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
}
