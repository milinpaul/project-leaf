//! Sorting and filtering a table (E09-S07): each type's conditions, what each change writes, and typing a filter.

use leaf_model::{CellValue, Database, FilterOp, PropertyType};

use super::super::tests::{NEW_YEAR, Sample, sample, view};
use super::{FilterInput, direction_label, filter_ops, op_label};
use crate::DatabaseEditor;

/// The titles of the rows the table shows, in order (row 1 has no title and shows as "Untitled").
fn shown(s: &mut Sample) -> Vec<String> {
    view(s).rows.into_iter().map(|r| r.title).collect()
}

const ALL: [&str; 3] = ["Lamp page", "Untitled", "Row 2"];

#[test]
fn each_type_offers_its_own_conditions_in_its_own_words() {
    let labels = |kind: PropertyType| -> Vec<&str> {
        filter_ops(&kind).iter().map(|op| op_label(op, &kind)).collect()
    };
    assert_eq!(
        labels(PropertyType::Text),
        [
            "contains",
            "does not contain",
            "is",
            "is not",
            "is empty",
            "is not empty"
        ]
    );
    assert_eq!(labels(PropertyType::Url), labels(PropertyType::Text));
    assert_eq!(
        labels(PropertyType::Number),
        ["=", "≠", ">", "<", "is empty", "is not empty"]
    );
    assert_eq!(
        labels(PropertyType::Select),
        ["is", "is not", "is empty", "is not empty"]
    );
    assert_eq!(
        labels(PropertyType::MultiSelect),
        ["has", "does not have", "is empty", "is not empty"]
    );
    assert_eq!(
        labels(PropertyType::Date),
        ["is", "is before", "is after", "is empty", "is not empty"]
    );
    assert_eq!(labels(PropertyType::Checkbox), ["is"]);
    assert_eq!(filter_ops(&PropertyType::Unknown("rollup".into())), Vec::new());
    assert_eq!(
        op_label(&FilterOp::Unknown("near".into()), &PropertyType::Text),
        "unknown condition"
    );
    assert_eq!(direction_label(&PropertyType::Number, true), "9 → 1");
    assert_eq!(direction_label(&PropertyType::Date, false), "Old → New");
    assert_eq!(direction_label(&PropertyType::Text, false), "A → Z");
    assert_eq!(direction_label(&PropertyType::Checkbox, true), "Ticked first");
    assert_eq!(direction_label(&PropertyType::Select, false), "Option order");
}

#[test]
fn sorts_are_added_turned_and_removed_each_one_undo_step() {
    let mut s = sample();
    assert_eq!(shown(&mut s), ALL);
    assert!(s.editor.add_sort(s.price).unwrap());
    assert!(!s.editor.add_sort(s.price).unwrap(), "already sorted by price");
    // Lamp 25, Chair 25 (tie keeps the database's order), Desk 120.5.
    assert_eq!(shown(&mut s), ["Lamp page", "Row 2", "Untitled"]);
    s.editor.set_sort_descending(0, true).unwrap();
    assert_eq!(shown(&mut s), ["Untitled", "Lamp page", "Row 2"]);
    s.editor.add_sort(s.name).unwrap();
    let bar = s.editor.query_bar(0).unwrap();
    assert_eq!(
        bar.sorts.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["Price", "Name"]
    );
    assert!(
        bar.sortable
            .iter()
            .all(|(id, ..)| *id != s.price && *id != s.name),
        "offered once"
    );
    assert_eq!(bar.filterable.len(), 7, "every column can be filtered");

    s.editor.remove_sort(0).unwrap();
    assert_eq!(s.editor.database().sort().len(), 1);
    assert!(s.editor.remove_sort(5).is_err());
    assert!(s.editor.undo().unwrap());
    assert_eq!(
        s.editor.database().sort().len(),
        2,
        "the remove undoes on its own"
    );
    assert!(s.editor.undo().unwrap());
    assert!(s.editor.undo().unwrap());
    assert_eq!(
        shown(&mut s),
        ["Lamp page", "Row 2", "Untitled"],
        "back to price ascending"
    );
}

#[test]
fn a_new_filter_narrows_nothing_until_it_has_a_value() {
    let mut s = sample();
    let index = s.editor.add_filter(s.status).unwrap();
    let bar = s.editor.query_bar(0).unwrap();
    let chip = &bar.filters[index];
    assert_eq!(
        (chip.name.as_str(), chip.op_label, chip.value_label.as_str()),
        ("Status", "is", "")
    );
    assert_eq!(chip.input, FilterInput::Option(None));
    assert_eq!(shown(&mut s), ALL);

    s.editor
        .set_filter_value(index, Some(CellValue::Option(s.options[1])))
        .unwrap();
    assert_eq!(shown(&mut s), ["Lamp page"]);
    assert_eq!(s.editor.query_bar(0).unwrap().filters[0].value_label, "Done");
    s.editor.set_filter_op(index, FilterOp::IsNot).unwrap();
    assert_eq!(shown(&mut s), ["Untitled", "Row 2"], "the value is kept");
    s.editor.set_filter_op(index, FilterOp::IsEmpty).unwrap();
    assert_eq!(shown(&mut s), ["Row 2"]);
    assert_eq!(
        s.editor.database().filters()[0].value,
        None,
        "an empty check drops the value"
    );
}

#[test]
fn typing_a_filter_shows_at_once_and_is_written_once() {
    let mut s = sample();
    let index = s.editor.add_filter(s.name).unwrap();
    let written = s.editor.database().filters();
    for typed in ["c", "ch", "cha"] {
        assert!(s.editor.type_filter_text(index, typed).unwrap());
    }
    assert_eq!(shown(&mut s), ["Row 2"], "Chair, while typing");
    assert_eq!(s.editor.database().filters(), written, "nothing written yet");
    assert_eq!(
        s.editor.query_bar(0).unwrap().filters[0].value_label,
        "\u{201c}cha\u{201d}"
    );

    assert!(s.editor.commit_filter_text().unwrap());
    assert!(!s.editor.commit_filter_text().unwrap(), "nothing left to commit");
    assert_eq!(shown(&mut s), ["Row 2"]);
    assert!(s.editor.undo().unwrap());
    assert_eq!(
        s.editor.database().filters(),
        written,
        "the whole word is one undo step"
    );

    // A dropped draft leaves the database's filter.
    s.editor.type_filter_text(index, "desk").unwrap();
    s.editor.cancel_filter_text();
    assert_eq!(shown(&mut s), ALL);
}

#[test]
fn a_number_filter_that_doesnt_read_as_a_number_narrows_nothing() {
    let mut s = sample();
    let index = s.editor.add_filter(s.price).unwrap();
    s.editor.set_filter_op(index, FilterOp::Greater).unwrap();
    assert!(!s.editor.type_filter_text(index, "lots").unwrap());
    assert_eq!(shown(&mut s), ALL);
    assert!(s.editor.type_filter_text(index, " 30 ").unwrap());
    assert_eq!(shown(&mut s), ["Untitled"], "the desk");
    s.editor.commit_filter_text().unwrap();
    let chip = &s.editor.query_bar(0).unwrap().filters[0];
    assert_eq!((chip.op_label, chip.value_label.as_str()), (">", "30"));
    assert_eq!(chip.input, FilterInput::Number("30".into()));
}

#[test]
fn every_type_filters_by_its_conditions() {
    let mut s = sample();
    // Multi-select: has metal.
    let tags = s.editor.add_filter(s.tags).unwrap();
    s.editor
        .set_filter_value(tags, Some(CellValue::Option(s.options[3])))
        .unwrap();
    assert_eq!(shown(&mut s), ["Untitled"]);
    s.editor.set_filter_op(tags, FilterOp::DoesNotContain).unwrap();
    assert_eq!(shown(&mut s), ["Lamp page", "Row 2"]);
    s.editor.remove_filter(tags).unwrap();

    // Date: before and after the new year, and empty.
    let due = s.editor.add_filter(s.due).unwrap();
    s.editor.set_filter_op(due, FilterOp::Less).unwrap();
    s.editor
        .set_filter_value(due, Some(CellValue::Date(NEW_YEAR + 1)))
        .unwrap();
    assert_eq!(shown(&mut s), ["Lamp page"]);
    assert_eq!(s.editor.query_bar(0).unwrap().filters[0].op_label, "is before");
    s.editor.set_filter_op(due, FilterOp::Greater).unwrap();
    assert_eq!(shown(&mut s), Vec::<String>::new());
    assert_eq!(
        s.editor.query_bar(0).unwrap().filters[0].input,
        FilterInput::Date(Some(NEW_YEAR + 1))
    );
    s.editor.set_filter_op(due, FilterOp::IsNotEmpty).unwrap();
    assert_eq!(shown(&mut s), ["Lamp page"]);
    s.editor.remove_filter(due).unwrap();

    // Checkbox: a new filter is "is ticked"; it can be turned to "not ticked".
    let done = s.editor.add_filter(s.done).unwrap();
    assert_eq!(
        s.editor.query_bar(0).unwrap().filters[0].input,
        FilterInput::Checkbox(true)
    );
    assert_eq!(shown(&mut s), ["Lamp page"]);
    s.editor
        .set_filter_value(done, Some(CellValue::Checkbox(false)))
        .unwrap();
    assert_eq!(shown(&mut s), ["Untitled", "Row 2"]);
    assert_eq!(
        s.editor.query_bar(0).unwrap().filters[0].value_label,
        "not ticked"
    );
    s.editor.remove_filter(done).unwrap();

    // URL: contains, case aside.
    let link = s.editor.add_filter(s.link).unwrap();
    s.editor.type_filter_text(link, "EXAMPLE").unwrap();
    s.editor.commit_filter_text().unwrap();
    assert_eq!(shown(&mut s), ["Row 2"]);
    s.editor.set_filter_op(link, FilterOp::DoesNotContain).unwrap();
    assert_eq!(shown(&mut s), ["Lamp page", "Untitled"]);
    assert!(s.editor.remove_filter(9).is_err());
}

#[test]
fn sorts_and_filters_are_saved_with_the_database() {
    let mut s = sample();
    s.editor.add_sort(s.price).unwrap();
    let f = s.editor.add_filter(s.status).unwrap();
    s.editor
        .set_filter_value(f, Some(CellValue::Option(s.options[0])))
        .unwrap();
    let bytes = s.editor.database().to_bytes().unwrap();
    let reopened = DatabaseEditor::new(Database::from_bytes(&bytes).unwrap());
    let bar = reopened.query_bar(0).unwrap();
    assert_eq!(bar.sorts.len(), 1);
    assert_eq!(bar.filters[0].value_label, "To do");
}

#[test]
fn a_filter_on_a_deleted_column_is_left_out_of_the_bar() {
    let mut s = sample();
    s.editor.add_filter(s.link).unwrap();
    s.editor.add_sort(s.link).unwrap();
    s.editor.delete_column(s.link).unwrap();
    let bar = s.editor.query_bar(0).unwrap();
    assert!(bar.filters.is_empty() && bar.sorts.is_empty());
    assert_eq!(shown(&mut s), ALL);
}

#[test]
fn undo_drops_a_filter_being_typed() {
    let mut s = sample();
    let index = s.editor.add_filter(s.name).unwrap();
    s.editor.type_filter_text(index, "lamp").unwrap();
    assert!(s.editor.undo().unwrap(), "takes the filter away");
    assert_eq!(shown(&mut s), ALL, "and its draft with it");
    assert!(!s.editor.commit_filter_text().unwrap());
}
