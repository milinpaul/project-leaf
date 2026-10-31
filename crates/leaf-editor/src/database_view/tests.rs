//! The table view of a database (E09-S04): what cells show, sorting and filtering, and reading only what is needed.

use std::collections::BTreeSet;

use leaf_core::{ColumnId, OptionId, PageId};
use leaf_model::{
    CellValue, Database, DatabaseCommand, DatabaseOutcome, Filter, FilterOp, PropertyType, Sort,
};

use super::{CellDisplay, DatabaseEditor, TableView};

/// 1 Jan 2026, 12:00 UTC.
pub(super) const NEW_YEAR: i64 = 1_767_268_800_000;

pub(super) struct Sample {
    pub(super) editor: DatabaseEditor,
    pub(super) name: ColumnId,
    pub(super) price: ColumnId,
    pub(super) status: ColumnId,
    pub(super) tags: ColumnId,
    pub(super) due: ColumnId,
    pub(super) done: ColumnId,
    pub(super) link: ColumnId,
    pub(super) rows: Vec<PageId>,
    pub(super) options: Vec<OptionId>,
}

fn column(db: &mut Database, name: &str, kind: PropertyType) -> ColumnId {
    let index = db.column_ids().len();
    match db.apply(DatabaseCommand::AddColumn {
        index,
        name: name.into(),
        kind,
    }) {
        Ok(DatabaseOutcome::ColumnCreated(id)) => id,
        other => panic!("expected a column, got {other:?}"),
    }
}

fn option(db: &mut Database, column: ColumnId, name: &str, color: &str) -> OptionId {
    match db.apply(DatabaseCommand::AddOption {
        column,
        name: name.into(),
        color: color.into(),
    }) {
        Ok(DatabaseOutcome::OptionCreated(id)) => id,
        other => panic!("expected an option, got {other:?}"),
    }
}

fn set(db: &mut DatabaseEditor, row: PageId, column: ColumnId, value: CellValue) {
    db.apply(DatabaseCommand::SetCell {
        row,
        column,
        value: Some(value),
    })
    .unwrap();
}

/// Three rows: a lamp, a desk and a chair, with every type filled in for some of them.
pub(super) fn sample() -> Sample {
    let mut db = Database::new();
    db.set_undo_merge_interval(0);
    let name = column(&mut db, "Name", PropertyType::Text);
    let price = column(&mut db, "Price", PropertyType::Number);
    let status = column(&mut db, "Status", PropertyType::Select);
    let tags = column(&mut db, "Tags", PropertyType::MultiSelect);
    let due = column(&mut db, "Due", PropertyType::Date);
    let done = column(&mut db, "Done", PropertyType::Checkbox);
    let link = column(&mut db, "Link", PropertyType::Url);
    let options = vec![
        option(&mut db, status, "To do", "gray"),
        option(&mut db, status, "Done", "green"),
        option(&mut db, tags, "wood", "brown"),
        option(&mut db, tags, "metal", "blue"),
    ];
    let rows: Vec<PageId> = (0..3)
        .map(|index| {
            let row = PageId::new();
            db.apply(DatabaseCommand::AddRow { row, index }).unwrap();
            row
        })
        .collect();
    let mut editor = DatabaseEditor::new(db);
    for (row, (text, cost)) in rows
        .iter()
        .zip([("Lamp", 25.0), ("Desk", 120.5), ("Chair", 25.0)])
    {
        set(&mut editor, *row, name, CellValue::Text(text.into()));
        set(&mut editor, *row, price, CellValue::Number(cost));
    }
    set(&mut editor, rows[0], status, CellValue::Option(options[1]));
    set(&mut editor, rows[1], status, CellValue::Option(options[0]));
    set(
        &mut editor,
        rows[1],
        tags,
        CellValue::Options(BTreeSet::from([options[2], options[3]])),
    );
    set(&mut editor, rows[0], due, CellValue::Date(NEW_YEAR));
    set(&mut editor, rows[0], done, CellValue::Checkbox(true));
    set(
        &mut editor,
        rows[2],
        link,
        CellValue::Text("https://example.com".into()),
    );
    Sample {
        editor,
        name,
        price,
        status,
        tags,
        due,
        done,
        link,
        rows,
        options,
    }
}

pub(super) fn titles(rows: &[PageId]) -> impl Fn(PageId) -> String + '_ {
    move |row| match rows.iter().position(|&r| r == row) {
        Some(0) => "Lamp page".into(),
        Some(1) => String::new(),
        Some(i) => format!("Row {i}"),
        None => "?".into(),
    }
}

pub(super) fn view(s: &mut Sample) -> TableView {
    s.editor.read_more(usize::MAX).unwrap();
    s.editor.view(titles(&s.rows), 0).unwrap()
}

/// The table as text, one line per row: the title, then each cell.
fn picture(view: &TableView) -> String {
    let mut lines = vec![
        std::iter::once("title".to_owned())
            .chain(
                view.columns
                    .iter()
                    .map(|c| format!("{} ({}, {})", c.name, c.kind.type_name(), c.width)),
            )
            .collect::<Vec<_>>()
            .join(" | "),
    ];
    for row in &view.rows {
        let cells: Vec<String> = row
            .cells
            .iter()
            .map(|cell| match cell {
                CellDisplay::Empty => "·".into(),
                CellDisplay::Text(t)
                | CellDisplay::Number(t)
                | CellDisplay::Date(t)
                | CellDisplay::Url(t) => t.clone(),
                CellDisplay::Checkbox(on) => if *on { "[x]" } else { "[ ]" }.into(),
                CellDisplay::Options(chips) => chips
                    .iter()
                    .map(|c| format!("{}:{}", c.name, c.color))
                    .collect::<Vec<_>>()
                    .join(","),
                CellDisplay::Unknown => "?".into(),
            })
            .collect();
        lines.push(format!("{} | {}", row.title, cells.join(" | ")));
    }
    lines.join("\n")
}

fn order(view: &TableView) -> Vec<PageId> {
    view.rows.iter().map(|r| r.row).collect()
}

#[test]
fn every_type_shows_as_its_column_reads() {
    let mut s = sample();
    let view = view(&mut s);
    assert!(view.complete);
    assert_eq!(view.total_rows, 3);
    insta::assert_snapshot!(picture(&view), @r"
    title | Name (text, 180) | Price (number, 180) | Status (select, 180) | Tags (multi_select, 180) | Due (date, 180) | Done (checkbox, 180) | Link (url, 180)
    Lamp page | Lamp | 25 | Done:green | · | 1 Jan 2026 | [x] | ·
    Untitled | Desk | 120.5 | To do:gray | wood:brown,metal:blue | · | [ ] | ·
    Row 2 | Chair | 25 | · | · | · | [ ] | https://example.com
    ");
    // Dates follow the device's time zone.
    let east = s.editor.view(titles(&s.rows), 13 * 60).unwrap();
    assert_eq!(east.rows[0].cells[4], CellDisplay::Date("2 Jan 2026".into()));
}

#[test]
fn hidden_columns_are_left_out_and_widths_come_through() {
    let mut s = sample();
    for command in [
        DatabaseCommand::SetColumnHidden {
            column: s.tags,
            hidden: true,
        },
        DatabaseCommand::SetColumnWidth {
            column: s.name,
            width: Some(260.0),
        },
    ] {
        s.editor.apply(command).unwrap();
    }
    let view = view(&mut s);
    let names: Vec<(&str, f64)> = view.columns.iter().map(|c| (c.name.as_str(), c.width)).collect();
    assert_eq!(
        names,
        [
            ("Name", 260.0),
            ("Price", 180.0),
            ("Status", 180.0),
            ("Due", 180.0),
            ("Done", 180.0),
            ("Link", 180.0)
        ]
    );
    assert!(view.rows.iter().all(|r| r.cells.len() == 6));
}

#[test]
fn a_retyped_column_shows_stored_values_converted_where_they_fit() {
    let mut s = sample();
    s.editor
        .apply(DatabaseCommand::SetColumnType {
            column: s.price,
            kind: PropertyType::Text,
        })
        .unwrap();
    s.editor
        .apply(DatabaseCommand::SetColumnType {
            column: s.name,
            kind: PropertyType::Number,
        })
        .unwrap();
    set(&mut s.editor, s.rows[2], s.name, CellValue::Number(7.0));
    s.editor
        .apply(DatabaseCommand::SetColumnType {
            column: s.status,
            kind: PropertyType::Text,
        })
        .unwrap();
    s.editor
        .apply(DatabaseCommand::SetColumnType {
            column: s.done,
            kind: PropertyType::Text,
        })
        .unwrap();
    let view = view(&mut s);
    let cell = |row: usize, column: usize| view.rows[row].cells[column].clone();
    assert_eq!(cell(0, 1), CellDisplay::Text("25".into()), "a number as text");
    assert_eq!(
        cell(0, 0),
        CellDisplay::Empty,
        "text that isn't a number shows empty"
    );
    assert_eq!(cell(2, 0), CellDisplay::Number("7".into()));
    assert_eq!(
        cell(0, 2),
        CellDisplay::Text("Done".into()),
        "an option by its name"
    );
    assert_eq!(cell(0, 5), CellDisplay::Text("Yes".into()));
    // The stored value is still the old one, so changing back shows it again.
    s.editor
        .apply(DatabaseCommand::SetColumnType {
            column: s.price,
            kind: PropertyType::Number,
        })
        .unwrap();
    assert_eq!(
        s.editor.view(titles(&s.rows), 0).unwrap().rows[0].cells[1],
        CellDisplay::Number("25".into())
    );
}

#[test]
fn sorting_is_stable_puts_empty_cells_last_and_runs_either_way() {
    let mut s = sample();
    let [lamp, desk, chair] = [s.rows[0], s.rows[1], s.rows[2]];
    let sort = |s: &mut Sample, sorts: Vec<Sort>| {
        s.editor.apply(DatabaseCommand::SetSort(sorts)).unwrap();
        order(&view(s))
    };
    let (price, name, status, due, done) = (s.price, s.name, s.status, s.due, s.done);
    assert_eq!(
        sort(&mut s, vec![Sort::new(price, false)]),
        [lamp, chair, desk],
        "ties keep row order"
    );
    assert_eq!(sort(&mut s, vec![Sort::new(price, true)]), [desk, lamp, chair]);
    assert_eq!(
        sort(&mut s, vec![Sort::new(price, false), Sort::new(name, false)]),
        [chair, lamp, desk],
        "a second key breaks ties"
    );
    assert_eq!(
        sort(&mut s, vec![Sort::new(name, false)]),
        [chair, desk, lamp],
        "text, ignoring case"
    );
    assert_eq!(
        sort(&mut s, vec![Sort::new(status, false)]),
        [desk, lamp, chair],
        "options by their place in the list; empty last"
    );
    assert_eq!(
        sort(&mut s, vec![Sort::new(status, true)]),
        [lamp, desk, chair],
        "empty last either way"
    );
    assert_eq!(sort(&mut s, vec![Sort::new(due, true)]), [lamp, desk, chair]);
    assert_eq!(sort(&mut s, vec![Sort::new(done, true)]), [lamp, desk, chair]);
    assert_eq!(
        sort(&mut s, vec![Sort::new(ColumnId::new(), false)]),
        [lamp, desk, chair],
        "a sort on a deleted column is ignored"
    );
}

#[test]
fn filters_narrow_the_rows_and_unknown_ones_are_ignored() {
    let mut s = sample();
    let [lamp, desk, chair] = [s.rows[0], s.rows[1], s.rows[2]];
    let filter = |s: &mut Sample, filters: Vec<Filter>| {
        s.editor.apply(DatabaseCommand::SetFilters(filters)).unwrap();
        order(&view(s))
    };
    let (name, price, status, tags, due, done, link) =
        (s.name, s.price, s.status, s.tags, s.due, s.done, s.link);
    let o = s.options.clone();
    let text = |t: &str| Some(CellValue::Text(t.into()));
    let cases: Vec<(Filter, Vec<PageId>)> = vec![
        (
            Filter::new(name, FilterOp::Contains, text("A")),
            vec![lamp, chair],
        ),
        (Filter::new(name, FilterOp::DoesNotContain, text("a")), vec![desk]),
        (Filter::new(name, FilterOp::Is, text("desk")), vec![desk]),
        (
            Filter::new(name, FilterOp::IsNot, text("desk")),
            vec![lamp, chair],
        ),
        (
            Filter::new(price, FilterOp::Greater, Some(CellValue::Number(30.0))),
            vec![desk],
        ),
        (
            Filter::new(price, FilterOp::Less, Some(CellValue::Number(30.0))),
            vec![lamp, chair],
        ),
        (
            Filter::new(status, FilterOp::Is, Some(CellValue::Option(o[1]))),
            vec![lamp],
        ),
        (Filter::new(status, FilterOp::IsEmpty, None), vec![chair]),
        (
            Filter::new(
                tags,
                FilterOp::Contains,
                Some(CellValue::Options(BTreeSet::from([o[3]]))),
            ),
            vec![desk],
        ),
        (
            Filter::new(
                tags,
                FilterOp::IsNot,
                Some(CellValue::Options(BTreeSet::from([o[3]]))),
            ),
            vec![lamp, chair],
        ),
        (
            Filter::new(due, FilterOp::Less, Some(CellValue::Date(NEW_YEAR + 1))),
            vec![lamp],
        ),
        (Filter::new(done, FilterOp::IsNotEmpty, None), vec![lamp]),
        (Filter::new(done, FilterOp::IsEmpty, None), vec![desk, chair]),
        (Filter::new(link, FilterOp::IsNotEmpty, None), vec![chair]),
        (
            Filter::new(name, FilterOp::Contains, None),
            vec![lamp, desk, chair],
        ),
        (
            Filter::new(name, FilterOp::Unknown("matches_regex".into()), text("x")),
            vec![lamp, desk, chair],
        ),
        (
            Filter::new(ColumnId::new(), FilterOp::IsEmpty, None),
            vec![lamp, desk, chair],
        ),
    ];
    for (one, expected) in cases {
        assert_eq!(filter(&mut s, vec![one.clone()]), expected, "{one:?}");
    }
    // Several filters must all match.
    assert_eq!(
        filter(
            &mut s,
            vec![
                Filter::new(price, FilterOp::Less, Some(CellValue::Number(30.0))),
                Filter::new(name, FilterOp::Contains, text("ch")),
            ]
        ),
        [chair]
    );
}

#[test]
fn the_first_screen_reads_only_its_rows_and_an_edit_reads_only_its_row() {
    let mut db = Database::new();
    let name = column(&mut db, "Name", PropertyType::Text);
    let rows: Vec<PageId> = (0..500)
        .map(|index| {
            let row = PageId::new();
            db.apply(DatabaseCommand::AddRow { row, index }).unwrap();
            db.apply(DatabaseCommand::SetCell {
                row,
                column: name,
                value: Some(CellValue::Text(format!("item {index}"))),
            })
            .unwrap();
            row
        })
        .collect();
    let mut editor = DatabaseEditor::new(db);
    assert_eq!(editor.rows_read(), 0, "opening reads no cells");
    assert!(!editor.read_more(super::FIRST_SCREEN_ROWS).unwrap());
    let view = editor.view(|_| String::new(), 0).unwrap();
    assert_eq!(editor.rows_read(), 50);
    assert_eq!(view.rows.len(), 50);
    assert_eq!(view.total_rows, 500);
    assert!(!view.complete);

    set(&mut editor, rows[3], name, CellValue::Text("changed".into()));
    let view = editor.view(|_| String::new(), 0).unwrap();
    assert_eq!(editor.rows_read(), 51, "only the edited row is read again");
    assert_eq!(view.rows[3].cells[0], CellDisplay::Text("changed".into()));
    // A row edited before it was ever read isn't read for that; it comes with its chunk.
    set(&mut editor, rows[400], name, CellValue::Text("later".into()));
    editor.view(|_| String::new(), 0).unwrap();
    assert_eq!(editor.rows_read(), 51);

    while !editor.read_more(200).unwrap() {}
    assert_eq!(editor.rows_read(), 501);
    let view = editor.view(|_| String::new(), 0).unwrap();
    assert!(view.complete);
    assert_eq!(view.rows[400].cells[0], CellDisplay::Text("later".into()));
}

#[test]
fn rows_added_moved_deleted_undone_or_merged_show_without_reading_the_rest() {
    let mut s = sample();
    view(&mut s);
    let read = s.editor.rows_read();
    let new_row = PageId::new();
    s.editor
        .apply(DatabaseCommand::AddRow {
            row: new_row,
            index: 0,
        })
        .unwrap();
    let v = s.editor.view(titles(&s.rows), 0).unwrap();
    assert_eq!(order(&v)[0], new_row);
    assert_eq!(s.editor.rows_read(), read + 1, "only the new row");

    s.editor
        .apply(DatabaseCommand::DeleteRow { row: s.rows[1] })
        .unwrap();
    s.editor
        .apply(DatabaseCommand::MoveRow {
            row: s.rows[2],
            index: 0,
        })
        .unwrap();
    let v = s.editor.view(titles(&s.rows), 0).unwrap();
    assert_eq!(order(&v), [s.rows[2], new_row, s.rows[0]]);
    assert_eq!(s.editor.rows_read(), read + 1);

    assert!(s.editor.undo().unwrap());
    assert!(s.editor.undo().unwrap());
    let v = s.editor.view(titles(&s.rows), 0).unwrap();
    assert_eq!(order(&v), [new_row, s.rows[0], s.rows[1], s.rows[2]]);

    // A change merged from another copy is picked up the same way.
    let mut other = Database::from_bytes(&s.editor.database().to_bytes().unwrap()).unwrap();
    other
        .apply(DatabaseCommand::SetCell {
            row: s.rows[0],
            column: s.name,
            value: Some(CellValue::Text("Lamp (merged)".into())),
        })
        .unwrap();
    let bytes = other.to_bytes().unwrap();
    let mut db = Database::from_bytes(&s.editor.database().to_bytes().unwrap()).unwrap();
    db.import_changes(&bytes).unwrap();
    let mut merged = DatabaseEditor::new(db);
    merged.read_more(usize::MAX).unwrap();
    let v = merged.view(titles(&s.rows), 0).unwrap();
    assert_eq!(v.rows[1].cells[0], CellDisplay::Text("Lamp (merged)".into()));
}

#[test]
fn every_conversion_between_a_stored_value_and_a_column_type() {
    use super::display::display;
    use leaf_model::{Column, SelectOption};
    let red = OptionId::new();
    let column = |kind: PropertyType| Column {
        id: ColumnId::new(),
        name: "c".into(),
        kind,
        hidden: false,
        width: None,
        options: vec![SelectOption {
            id: red,
            name: "Red".into(),
            color: "red".into(),
        }],
    };
    let shows = |value: Option<CellValue>, kind: PropertyType| display(value.as_ref(), &column(kind), 0);
    let text = |t: &str| Some(CellValue::Text(t.into()));
    // Into text: every known value reads as something; an unknown one doesn't.
    assert_eq!(
        shows(Some(CellValue::Date(NEW_YEAR)), PropertyType::Text),
        CellDisplay::Text("1 Jan 2026".into())
    );
    assert_eq!(
        shows(Some(CellValue::Checkbox(false)), PropertyType::Text),
        CellDisplay::Text("No".into())
    );
    assert_eq!(
        shows(Some(CellValue::Options(BTreeSet::from([red]))), PropertyType::Url),
        CellDisplay::Url("Red".into())
    );
    assert_eq!(
        shows(text(&red.to_string()), PropertyType::Text),
        CellDisplay::Text("Red".into())
    );
    assert_eq!(
        shows(Some(CellValue::Unknown), PropertyType::Text),
        CellDisplay::Empty
    );
    assert_eq!(shows(text(""), PropertyType::Text), CellDisplay::Empty);
    // Into a number or a date: only what reads as one.
    assert_eq!(
        shows(text(" 2.5 "), PropertyType::Number),
        CellDisplay::Number("2.5".into())
    );
    assert_eq!(
        shows(Some(CellValue::Checkbox(true)), PropertyType::Number),
        CellDisplay::Empty
    );
    assert_eq!(shows(text("2026-01-01"), PropertyType::Date), CellDisplay::Empty);
    // Into options: an option's ID as text, a multi-select's first option for a select.
    assert_eq!(
        shows(text(&red.to_string()), PropertyType::Select),
        CellDisplay::Options(vec![super::Chip {
            option: red,
            name: "Red".into(),
            color: "red".into(),
        }])
    );
    assert_eq!(
        shows(
            Some(CellValue::Option(OptionId::new())),
            PropertyType::MultiSelect
        ),
        CellDisplay::Empty
    );
    assert_eq!(
        shows(Some(CellValue::Number(1.0)), PropertyType::Select),
        CellDisplay::Empty
    );
    // A checkbox always shows; a newer version's type shows as such.
    assert_eq!(shows(None, PropertyType::Checkbox), CellDisplay::Checkbox(false));
    assert_eq!(
        shows(text("x"), PropertyType::Checkbox),
        CellDisplay::Checkbox(false)
    );
    let formula = PropertyType::Unknown("formula".into());
    assert_eq!(
        shows(Some(CellValue::Unknown), formula.clone()),
        CellDisplay::Unknown
    );
    assert_eq!(shows(None, formula), CellDisplay::Empty);
    assert_eq!(shows(None, PropertyType::Text), CellDisplay::Empty);
    assert_eq!(super::date_label(-1, 0), "31 Dec 1969");
}
