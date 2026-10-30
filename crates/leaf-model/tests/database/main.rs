//! Property tests for the database document (E09-S02): random command sequences keep every invariant and
//! survive a save; two devices editing at once converge; undo and redo walk back and forth through exactly the
//! states the commands went through. Run with `PROPTEST_CASES=10000` for the long run.

mod convergence;
mod history;

use std::collections::BTreeSet;

use leaf_core::{ColumnId, PageId};
use leaf_model::{CellValue, Database, DatabaseCommand, Filter, FilterOp, PropertyType, Sort};
use proptest::prelude::*;

/// One random edit, chosen by small numbers that pick among what the database has when it is applied.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Step {
    AddColumn(u8, u8),
    RenameColumn(u8, u8),
    SetColumnType(u8, u8),
    MoveColumn(u8, u8),
    HideColumn(u8, bool),
    SetWidth(u8, Option<u8>),
    DeleteColumn(u8),
    AddOption(u8, u8),
    RenameOption(u8, u8, u8),
    DeleteOption(u8, u8),
    AddRow(u8),
    MoveRow(u8, u8),
    DeleteRow(u8),
    SetCell(u8, u8, u8),
    ClearCell(u8, u8),
    SetSort(u8, bool),
    SetFilters(u8),
}

pub(crate) fn step() -> impl Strategy<Value = Step> {
    let n = any::<u8>;
    prop_oneof![
        3 => (n(), n()).prop_map(|(a, b)| Step::AddColumn(a, b)),
        1 => (n(), n()).prop_map(|(a, b)| Step::RenameColumn(a, b)),
        1 => (n(), n()).prop_map(|(a, b)| Step::SetColumnType(a, b)),
        1 => (n(), n()).prop_map(|(a, b)| Step::MoveColumn(a, b)),
        1 => (n(), any::<bool>()).prop_map(|(a, b)| Step::HideColumn(a, b)),
        1 => (n(), proptest::option::of(n())).prop_map(|(a, b)| Step::SetWidth(a, b)),
        1 => n().prop_map(Step::DeleteColumn),
        2 => (n(), n()).prop_map(|(a, b)| Step::AddOption(a, b)),
        1 => (n(), n(), n()).prop_map(|(a, b, c)| Step::RenameOption(a, b, c)),
        1 => (n(), n()).prop_map(|(a, b)| Step::DeleteOption(a, b)),
        3 => n().prop_map(Step::AddRow),
        1 => (n(), n()).prop_map(|(a, b)| Step::MoveRow(a, b)),
        1 => n().prop_map(Step::DeleteRow),
        5 => (n(), n(), n()).prop_map(|(a, b, c)| Step::SetCell(a, b, c)),
        1 => (n(), n()).prop_map(|(a, b)| Step::ClearCell(a, b)),
        1 => (n(), any::<bool>()).prop_map(|(a, b)| Step::SetSort(a, b)),
        1 => n().prop_map(Step::SetFilters),
    ]
}

pub(crate) fn steps(max: usize) -> impl Strategy<Value = Vec<Step>> {
    proptest::collection::vec(step(), 0..max)
}

const KINDS: [PropertyType; 7] = [
    PropertyType::Text,
    PropertyType::Number,
    PropertyType::Select,
    PropertyType::MultiSelect,
    PropertyType::Date,
    PropertyType::Checkbox,
    PropertyType::Url,
];
const NAMES: [&str; 4] = ["Name", "Price", "Status", "Notes"];

fn pick<T: Copy>(items: &[T], seed: u8) -> Option<T> {
    (!items.is_empty()).then(|| items[usize::from(seed) % items.len()])
}

fn kind(seed: u8) -> PropertyType {
    KINDS[usize::from(seed) % KINDS.len()].clone()
}

fn column(db: &Database, seed: u8) -> Option<ColumnId> {
    pick(&db.column_ids(), seed)
}

fn row(db: &Database, seed: u8) -> Option<PageId> {
    pick(&db.rows(), seed)
}

/// A value that fits `column`'s type, or `None` when it has no options to pick.
fn value_for(db: &Database, column: ColumnId, seed: u8) -> Option<CellValue> {
    let col = db.column(column).ok()?;
    let options: Vec<_> = col.options.iter().map(|o| o.id).collect();
    Some(match col.kind {
        PropertyType::Text | PropertyType::Url => CellValue::Text(format!("v{}", seed % 5)),
        PropertyType::Number => CellValue::Number(f64::from(seed % 7)),
        PropertyType::Date => CellValue::Date(i64::from(seed) * 1000),
        PropertyType::Checkbox => CellValue::Checkbox(seed.is_multiple_of(2)),
        PropertyType::Select => CellValue::Option(pick(&options, seed)?),
        PropertyType::MultiSelect => CellValue::Options(
            options
                .iter()
                .enumerate()
                .filter(|(i, _)| seed >> (i % 8) & 1 == 1)
                .map(|(_, id)| *id)
                .collect::<BTreeSet<_>>(),
        ),
        PropertyType::Unknown(_) => return None,
    })
}

/// The command `step` means on `db` as it is, or `None` when there is nothing to pick.
pub(crate) fn command(step: Step, db: &Database) -> Option<DatabaseCommand> {
    let columns = db.column_ids().len();
    let rows = db.rows().len();
    Some(match step {
        Step::AddColumn(at, k) => DatabaseCommand::AddColumn {
            index: usize::from(at) % (columns + 1),
            name: NAMES[usize::from(k) % NAMES.len()].into(),
            kind: kind(k),
        },
        Step::RenameColumn(c, n) => DatabaseCommand::RenameColumn {
            column: column(db, c)?,
            name: NAMES[usize::from(n) % NAMES.len()].into(),
        },
        Step::SetColumnType(c, k) => DatabaseCommand::SetColumnType {
            column: column(db, c)?,
            kind: kind(k),
        },
        Step::MoveColumn(c, at) => DatabaseCommand::MoveColumn {
            column: column(db, c)?,
            index: usize::from(at) % columns.max(1),
        },
        Step::HideColumn(c, hidden) => DatabaseCommand::SetColumnHidden {
            column: column(db, c)?,
            hidden,
        },
        Step::SetWidth(c, w) => DatabaseCommand::SetColumnWidth {
            column: column(db, c)?,
            width: w.map(|w| f64::from(w) + 40.0),
        },
        Step::DeleteColumn(c) => DatabaseCommand::DeleteColumn {
            column: column(db, c)?,
        },
        Step::AddOption(c, n) => DatabaseCommand::AddOption {
            column: column(db, c)?,
            name: format!("o{}", n % 4),
            color: ["gray", "red", "green"][usize::from(n) % 3].into(),
        },
        Step::RenameOption(c, o, n) => {
            let column = column(db, c)?;
            let options: Vec<_> = db.column(column).ok()?.options.iter().map(|o| o.id).collect();
            DatabaseCommand::RenameOption {
                column,
                option: pick(&options, o)?,
                name: format!("o{}", n % 4),
            }
        }
        Step::DeleteOption(c, o) => {
            let column = column(db, c)?;
            let options: Vec<_> = db.column(column).ok()?.options.iter().map(|o| o.id).collect();
            DatabaseCommand::DeleteOption {
                column,
                option: pick(&options, o)?,
            }
        }
        // Rows are new pages: a page is never a row twice.
        Step::AddRow(at) => DatabaseCommand::AddRow {
            row: PageId::new(),
            index: usize::from(at) % (rows + 1),
        },
        Step::MoveRow(r, at) => DatabaseCommand::MoveRow {
            row: row(db, r)?,
            index: usize::from(at) % rows.max(1),
        },
        Step::DeleteRow(r) => DatabaseCommand::DeleteRow { row: row(db, r)? },
        Step::SetCell(r, c, v) => {
            let column = column(db, c)?;
            DatabaseCommand::SetCell {
                row: row(db, r)?,
                column,
                value: Some(value_for(db, column, v)?),
            }
        }
        Step::ClearCell(r, c) => DatabaseCommand::SetCell {
            row: row(db, r)?,
            column: column(db, c)?,
            value: None,
        },
        Step::SetSort(c, descending) => DatabaseCommand::SetSort(vec![Sort::new(column(db, c)?, descending)]),
        Step::SetFilters(c) => {
            let column = column(db, c)?;
            DatabaseCommand::SetFilters(vec![Filter::new(column, FilterOp::IsNotEmpty, None)])
        }
    })
}

/// Applies `steps`; refused commands are fine, they change nothing.
pub(crate) fn edit(db: &mut Database, steps: Vec<Step>) {
    for step in steps {
        if let Some(command) = command(step, db) {
            let _ = db.apply(command);
        }
    }
}

/// Everything a reader can see, as text, to compare two databases.
pub(crate) fn outline(db: &Database) -> String {
    let columns = match db.columns() {
        Ok(columns) => columns,
        Err(error) => return format!("unreadable columns: {error}"),
    };
    let mut lines: Vec<String> = columns
        .iter()
        .map(|column| {
            let options: Vec<String> = column
                .options
                .iter()
                .map(|o| format!("{}:{}:{}", o.id, o.name, o.color))
                .collect();
            format!(
                "column {} {:?} {} hidden={} width={:?} options={options:?}",
                column.id,
                column.name,
                column.kind.type_name(),
                column.hidden,
                column.width,
            )
        })
        .collect();
    for row in db.rows() {
        let cells: Vec<String> = db
            .column_ids()
            .into_iter()
            .map(|column| format!("{:?}", db.cell(row, column)))
            .collect();
        lines.push(format!("row {row} {}", cells.join(" ")));
    }
    lines.push(format!("sort {:?}", db.sort()));
    lines.push(format!("filters {:?}", db.filters()));
    lines.join("\n")
}

proptest! {
    #[test]
    fn random_commands_keep_every_invariant_and_survive_a_save(steps in steps(40)) {
        let mut db = Database::new();
        for step in steps {
            if let Some(command) = command(step, &db) {
                let before = db.to_bytes().unwrap();
                if db.apply(command.clone()).is_err() {
                    prop_assert_eq!(db.to_bytes().unwrap(), before, "a refused {:?} wrote", command);
                }
                if let Err(broken) = db.check_invariants() {
                    prop_assert!(false, "after {:?}: {:?}", command, broken);
                }
            }
        }
        let loaded = Database::from_bytes(&db.to_bytes().unwrap()).unwrap();
        prop_assert_eq!(outline(&loaded), outline(&db));
    }
}
