//! A row's properties on its page (E09-S08).

use leaf_core::PageId;
use leaf_model::{CellValue, PropertyType};

use super::super::tests::{NEW_YEAR, sample};
use crate::CellDisplay;

#[test]
fn every_column_is_listed_in_order_with_the_rows_value_as_the_table_shows_it() {
    let mut s = sample();
    s.editor.set_column_hidden(s.link, true).unwrap();
    let props = s.editor.row_properties(s.rows[0], 0).unwrap();
    let names: Vec<&str> = props.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["Name", "Price", "Status", "Tags", "Due", "Done", "Link"],
        "hidden ones too"
    );
    assert_eq!(props[0].display, CellDisplay::Text("Lamp".into()));
    assert_eq!(props[1].display, CellDisplay::Number("25".into()));
    assert!(matches!(&props[2].display, CellDisplay::Options(chips) if chips[0].name == "Done"));
    assert_eq!(props[3].display, CellDisplay::Empty);
    assert_eq!(props[4].kind, PropertyType::Date);
    assert!(matches!(&props[4].display, CellDisplay::Date(label) if label.contains("2026")));
    assert_eq!(props[5].display, CellDisplay::Checkbox(true));
}

#[test]
fn a_property_edited_on_the_page_is_what_the_table_shows() {
    let mut s = sample();
    let row = s.rows[2];
    s.editor.commit_text(row, s.price, "7.5").unwrap();
    s.editor.set_date(row, s.due, Some(NEW_YEAR)).unwrap();
    let props = s.editor.row_properties(row, 0).unwrap();
    assert_eq!(props[1].display, CellDisplay::Number("7.5".into()));
    assert_eq!(
        s.editor.database().cell(row, s.price).unwrap(),
        Some(CellValue::Number(7.5))
    );
    assert!(matches!(props[4].display, CellDisplay::Date(_)));
}

#[test]
fn a_row_the_database_doesnt_have_has_no_properties() {
    let mut s = sample();
    assert_eq!(s.editor.row_properties(PageId::new(), 0).unwrap(), Vec::new());
    let row = s.rows[0];
    s.editor
        .apply(leaf_model::DatabaseCommand::DeleteRow { row })
        .unwrap();
    assert_eq!(s.editor.row_properties(row, 0).unwrap(), Vec::new());
}
