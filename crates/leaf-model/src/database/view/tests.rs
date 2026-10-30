//! The view's sort and filters: set, read back, replaced whole, and a newer version's extras kept.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId};

use super::{Filter, FilterOp, Sort};
use crate::database::tests::{add_column, database};
use crate::database::{CellValue, DatabaseCommand, DatabaseOutcome, PropertyType};

#[test]
fn the_sort_and_filters_are_set_read_back_and_replaced_whole() {
    let mut db = database();
    let price = add_column(&mut db, "Price", PropertyType::Number);
    let status = add_column(&mut db, "Status", PropertyType::Select);
    let tags = add_column(&mut db, "Tags", PropertyType::MultiSelect);
    let done = OptionId::new();
    let sort = vec![Sort::new(price, true), Sort::new(status, false)];
    db.apply(DatabaseCommand::SetSort(sort.clone())).unwrap();
    assert_eq!(db.sort(), sort);
    assert_eq!(
        db.apply(DatabaseCommand::SetSort(sort.clone())).unwrap(),
        DatabaseOutcome::Unchanged
    );

    let filters = vec![
        Filter::new(price, FilterOp::Greater, Some(CellValue::Number(10.0))),
        Filter::new(status, FilterOp::Is, Some(CellValue::Option(done))),
        Filter::new(
            tags,
            FilterOp::Contains,
            Some(CellValue::Options(BTreeSet::from([done]))),
        ),
        Filter::new(price, FilterOp::IsNotEmpty, None),
    ];
    db.apply(DatabaseCommand::SetFilters(filters.clone())).unwrap();
    assert_eq!(db.filters(), filters);
    db.apply(DatabaseCommand::SetFilters(Vec::new())).unwrap();
    assert_eq!(db.filters(), Vec::new());
    assert_eq!(db.sort(), sort, "filters and sort are separate");
    assert_eq!(db.check_invariants(), Ok(()));
}

#[test]
fn a_newer_versions_filter_condition_and_extra_keys_are_kept_when_the_list_is_rewritten() {
    let column = ColumnId::new();
    let stored = super::LoroValue::from(vec![super::LoroValue::from(
        [
            ("column".to_owned(), super::LoroValue::from(column.to_string())),
            ("op".to_owned(), "within_last_days".into()),
            ("value".to_owned(), 7_i64.into()),
            ("relative".to_owned(), true.into()),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>(),
    )]);
    let read = super::read_filters(Some(stored.clone()), |_| None);
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].op, FilterOp::Unknown("within_last_days".into()));
    assert_eq!(super::write_filters(&read), stored, "written back exactly");

    let sorted = super::LoroValue::from(vec![super::LoroValue::from(
        [
            ("column".to_owned(), super::LoroValue::from(column.to_string())),
            ("descending".to_owned(), false.into()),
            ("nulls".to_owned(), "first".into()),
        ]
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>(),
    )]);
    let read = super::read_sort(Some(sorted.clone()));
    assert_eq!(read[0].column, column);
    assert_eq!(super::write_sort(&read), sorted);
    // Entries without a column, or not maps at all, are skipped.
    assert_eq!(
        super::read_sort(Some(super::LoroValue::from(vec![super::LoroValue::from(1_i64)]))),
        Vec::new()
    );
    assert_eq!(super::read_sort(None), Vec::new());
}
