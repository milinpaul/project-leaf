//! Cells: each type takes its own values, options must be the column's, unknown types stay read-only, a retype
//! keeps stored values, and multi-select merges tag by tag.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId, PageId};

use super::{add_column, add_row, database};
use crate::ModelError;
use crate::database::{CellValue, Database, DatabaseCommand, DatabaseOutcome, PropertyType};

fn set(
    db: &mut Database,
    row: PageId,
    column: ColumnId,
    value: Option<CellValue>,
) -> Result<DatabaseOutcome, ModelError> {
    db.apply(DatabaseCommand::SetCell { row, column, value })
}

fn option(db: &mut Database, column: ColumnId, name: &str) -> OptionId {
    match db.apply(DatabaseCommand::AddOption {
        column,
        name: name.into(),
        color: "gray".into(),
    }) {
        Ok(DatabaseOutcome::OptionCreated(id)) => id,
        other => panic!("expected an option, got {other:?}"),
    }
}

#[test]
fn every_type_takes_its_own_values_and_refuses_others() {
    let mut db = database();
    let row = add_row(&mut db);
    let cases = [
        (
            PropertyType::Text,
            CellValue::Text("Lamp".into()),
            CellValue::Number(1.0),
        ),
        (
            PropertyType::Url,
            CellValue::Text("https://example.com".into()),
            CellValue::Checkbox(true),
        ),
        (
            PropertyType::Number,
            CellValue::Number(2.5),
            CellValue::Number(f64::NAN),
        ),
        (
            PropertyType::Date,
            CellValue::Date(1_790_000_000_000),
            CellValue::Text("today".into()),
        ),
        (
            PropertyType::Checkbox,
            CellValue::Checkbox(true),
            CellValue::Date(0),
        ),
    ];
    for (kind, good, bad) in cases {
        let name = kind.type_name().to_owned();
        let column = add_column(&mut db, &name, kind);
        assert_eq!(
            set(&mut db, row, column, Some(good.clone())).unwrap(),
            DatabaseOutcome::Done
        );
        assert_eq!(db.cell(row, column).unwrap(), Some(good.clone()));
        assert!(matches!(
            set(&mut db, row, column, Some(bad)),
            Err(ModelError::CellTypeMismatch { .. })
        ));
        assert_eq!(
            set(&mut db, row, column, Some(good)).unwrap(),
            DatabaseOutcome::Unchanged,
            "the same value again"
        );
        set(&mut db, row, column, None).unwrap();
        assert_eq!(db.cell(row, column).unwrap(), None);
    }
    assert!(matches!(
        set(&mut db, PageId::new(), ColumnId::new(), None),
        Err(ModelError::RowNotFound(_))
    ));
    assert!(matches!(
        db.cell(row, ColumnId::new()),
        Err(ModelError::ColumnNotFound(_))
    ));
    assert_eq!(db.check_invariants(), Ok(()));
}

#[test]
fn select_cells_take_only_the_columns_options() {
    let mut db = database();
    let row = add_row(&mut db);
    let status = add_column(&mut db, "Status", PropertyType::Select);
    let tags = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let done = option(&mut db, status, "Done");
    let red = option(&mut db, tags, "red");
    let blue = option(&mut db, tags, "blue");

    set(&mut db, row, status, Some(CellValue::Option(done))).unwrap();
    assert_eq!(db.cell(row, status).unwrap(), Some(CellValue::Option(done)));
    assert!(matches!(
        set(&mut db, row, status, Some(CellValue::Option(red))),
        Err(ModelError::OptionNotFound { .. })
    ));
    let both = BTreeSet::from([red, blue]);
    set(&mut db, row, tags, Some(CellValue::Options(both.clone()))).unwrap();
    assert_eq!(db.cell(row, tags).unwrap(), Some(CellValue::Options(both)));
    set(
        &mut db,
        row,
        tags,
        Some(CellValue::Options(BTreeSet::from([blue]))),
    )
    .unwrap();
    assert_eq!(
        db.cell(row, tags).unwrap(),
        Some(CellValue::Options(BTreeSet::from([blue])))
    );
    // Emptying a multi-select clears its options; the cell reads as an empty set.
    set(&mut db, row, tags, None).unwrap();
    assert_eq!(
        db.cell(row, tags).unwrap(),
        Some(CellValue::Options(BTreeSet::new()))
    );
    assert!(matches!(
        set(&mut db, row, status, Some(CellValue::Options(BTreeSet::new()))),
        Err(ModelError::CellTypeMismatch { .. })
    ));
}

#[test]
fn a_retype_keeps_stored_values_and_changing_back_shows_them_again() {
    let mut db = database();
    let row = add_row(&mut db);
    let price = add_column(&mut db, "Price", PropertyType::Number);
    set(&mut db, row, price, Some(CellValue::Number(12.0))).unwrap();
    db.apply(DatabaseCommand::SetColumnType {
        column: price,
        kind: PropertyType::Text,
    })
    .unwrap();
    // Still the stored number: the editor converts it for display (ADR-066 answer 3).
    assert_eq!(db.cell(row, price).unwrap(), Some(CellValue::Number(12.0)));
    db.apply(DatabaseCommand::SetColumnType {
        column: price,
        kind: PropertyType::Number,
    })
    .unwrap();
    assert_eq!(db.cell(row, price).unwrap(), Some(CellValue::Number(12.0)));
}

#[test]
fn tags_added_on_two_copies_are_all_kept() {
    let mut a = database();
    let row = add_row(&mut a);
    let tags = add_column(&mut a, "Tags", PropertyType::MultiSelect);
    let (red, blue, gold) = (
        option(&mut a, tags, "red"),
        option(&mut a, tags, "blue"),
        option(&mut a, tags, "gold"),
    );
    set(&mut a, row, tags, Some(CellValue::Options(BTreeSet::from([red])))).unwrap();
    let mut b = Database::from_bytes(&a.to_bytes().unwrap()).unwrap();
    set(
        &mut a,
        row,
        tags,
        Some(CellValue::Options(BTreeSet::from([red, blue]))),
    )
    .unwrap();
    set(
        &mut b,
        row,
        tags,
        Some(CellValue::Options(BTreeSet::from([gold]))),
    )
    .unwrap();
    let (from_a, from_b) = (a.to_bytes().unwrap(), b.to_bytes().unwrap());
    a.import_changes(&from_b).unwrap();
    b.import_changes(&from_a).unwrap();
    // B removed red and added gold; A added blue. All of it holds.
    let expected = Some(CellValue::Options(BTreeSet::from([blue, gold])));
    assert_eq!(a.cell(row, tags).unwrap(), expected);
    assert_eq!(b.cell(row, tags).unwrap(), expected);
}

#[test]
fn a_column_of_a_type_from_a_newer_version_is_read_only_and_kept() {
    // Build what a newer version would store: a column typed "formula" with its own settings and a cell.
    let newer = loro::LoroDoc::new();
    let column = ColumnId::new();
    let row = PageId::new();
    let schema = newer
        .get_map("schema")
        .insert_container(&column.to_string(), loro::LoroMap::new())
        .unwrap();
    schema.insert("name", "Total").unwrap();
    schema.insert("type", "formula").unwrap();
    schema.insert("expression", "prop(\"Price\") * 2").unwrap();
    let columns = newer.get_tree("columns");
    columns.enable_fractional_index(0);
    let node = columns.create(loro::TreeParentId::Root).unwrap();
    columns
        .get_meta(node)
        .unwrap()
        .insert("id", column.to_string())
        .unwrap();
    let rows = newer.get_tree("rows");
    rows.enable_fractional_index(0);
    let node = rows.create(loro::TreeParentId::Root).unwrap();
    rows.get_meta(node)
        .unwrap()
        .insert("id", row.to_string())
        .unwrap();
    let cell = newer
        .get_map("cells")
        .insert_container(&row.to_string(), loro::LoroMap::new())
        .unwrap()
        .insert_container(&column.to_string(), loro::LoroList::new())
        .unwrap();
    cell.push(42.0).unwrap();
    newer.commit();

    let mut db = Database::from_bytes(&newer.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    db.set_undo_merge_interval(0);
    assert_eq!(
        db.column(column).unwrap().kind,
        PropertyType::Unknown("formula".into())
    );
    assert_eq!(db.cell(row, column).unwrap(), Some(CellValue::Unknown));
    assert!(matches!(
        set(&mut db, row, column, Some(CellValue::Text("x".into()))),
        Err(ModelError::UnknownColumnType { .. })
    ));
    assert!(matches!(
        db.apply(DatabaseCommand::SetColumnType {
            column,
            kind: PropertyType::Text,
        }),
        Err(ModelError::UnknownColumnType { .. })
    ));
    // Renaming, hiding and moving touch only what this version knows.
    db.apply(DatabaseCommand::RenameColumn {
        column,
        name: "Total (EUR)".into(),
    })
    .unwrap();
    let saved = Database::from_bytes(&db.to_bytes().unwrap()).unwrap();
    assert_eq!(saved.column(column).unwrap().name, "Total (EUR)");
    let raw = loro::LoroDoc::new();
    raw.import(&db.to_bytes().unwrap()).unwrap();
    let deep = raw.get_deep_value();
    assert_eq!(
        at(&deep, &["schema", &column.to_string(), "expression"]),
        Some(loro::LoroValue::from("prop(\"Price\") * 2"))
    );
    assert_eq!(
        at(&deep, &["cells", &row.to_string(), &column.to_string()]),
        Some(loro::LoroValue::from(vec![loro::LoroValue::from(42.0)]))
    );
}

/// The value at `path` inside nested maps.
fn at(value: &loro::LoroValue, path: &[&str]) -> Option<loro::LoroValue> {
    let mut current = value.clone();
    for key in path {
        let loro::LoroValue::Map(map) = current else {
            return None;
        };
        current = map.get(*key)?.clone();
    }
    Some(current)
}
