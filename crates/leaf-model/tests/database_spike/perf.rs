//! How a big database behaves in this model: size on disk, load, reading every cell, sorting and filtering, and
//! one edit. A measurement, not a check, so it is ignored by default; run it in a release build:
//!
//! ```text
//! cargo test --release -p leaf-model --test database_spike -- --ignored --nocapture
//! ```
//!
//! It also measures the alternative of keeping each row's properties in the row's own page document, where a
//! table view has to load one document per row.

use std::time::{Duration, Instant};

use loro::{ExportMode, LoroDoc, LoroMap, LoroResult, LoroValue};

use super::{Db, at, export};

const ROWS: usize = 10_000;
const COLUMNS: [(&str, &str); 8] = [
    ("c-name", "text"),
    ("c-notes", "text"),
    ("c-price", "number"),
    ("c-status", "select"),
    ("c-tags", "multi_select"),
    ("c-due", "date"),
    ("c-done", "checkbox"),
    ("c-link", "url"),
];

/// A deterministic number from `i`, so runs are comparable.
fn spread(i: usize) -> u64 {
    (i as u64).wrapping_mul(2_654_435_761) % 100_000
}

fn fill(cells: &LoroMap, i: usize) -> LoroResult<()> {
    let n = spread(i);
    cells.insert("c-name", format!("Item {i}"))?;
    cells.insert(
        "c-notes",
        format!("Some notes about item {i}, a sentence or so long."),
    )?;
    #[expect(clippy::cast_precision_loss, reason = "small test numbers")]
    cells.insert("c-price", n as f64 / 100.0)?;
    cells.insert("c-status", format!("o-{}", n % 4))?;
    let tags = cells.insert_container("c-tags", LoroMap::new())?;
    tags.insert(&format!("o-tag{}", n % 7), true)?;
    tags.insert(&format!("o-tag{}", n % 5), true)?;
    #[expect(clippy::cast_possible_wrap, reason = "small test numbers")]
    cells.insert("c-due", 1_790_000_000_000_i64 + n as i64 * 60_000)?;
    cells.insert("c-done", n.is_multiple_of(3))?;
    cells.insert("c-link", format!("https://example.com/{n}"))?;
    Ok(())
}

fn build() -> LoroResult<Db> {
    let db = Db::new(1)?;
    for (id, kind) in COLUMNS {
        db.add_column(id, id.trim_start_matches("c-"), kind)?;
    }
    for i in 0..ROWS {
        let row = format!("row-{i:05}");
        db.rows().push(row.as_str())?;
        fill(&db.row(&row)?, i)?;
    }
    db.doc.commit();
    Ok(db)
}

/// One row as a table view holds it.
struct Row {
    id: String,
    values: Vec<Option<LoroValue>>,
}

fn read(db: &Db) -> Vec<Row> {
    let columns = db.column_ids();
    let cells = db.cells().get_deep_value();
    db.row_ids()
        .into_iter()
        .map(|id| Row {
            values: columns.iter().map(|c| at(&cells, &[&id, c])).collect(),
            id,
        })
        .collect()
}

fn number(value: Option<&LoroValue>) -> f64 {
    match value {
        Some(LoroValue::Double(d)) => *d,
        _ => f64::NEG_INFINITY,
    }
}

fn time<T>(label: &str, out: &mut Vec<(String, Duration)>, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let value = f();
    out.push((label.to_owned(), start.elapsed()));
    value
}

#[test]
#[ignore = "a measurement for ADR 0066; run with --release --ignored --nocapture"]
fn a_ten_thousand_row_database_loads_sorts_and_filters_quickly() -> LoroResult<()> {
    let mut times = Vec::new();
    let db = time("build (one commit)", &mut times, build)?;
    let snapshot = time("save (snapshot)", &mut times, || {
        export(&db.doc, ExportMode::Snapshot)
    })?;

    let loaded = Db::new(2)?;
    time("load", &mut times, || loaded.doc.import(&snapshot))?;
    // Where the read time goes: Loro building the whole value, then picking cells out of it.
    let fresh = Db::new(3)?;
    fresh.doc.import(&snapshot)?;
    time("read: Loro's deep value of all cells", &mut times, || {
        fresh.cells().get_deep_value()
    });
    let window = Db::new(4)?;
    window.doc.import(&snapshot)?;
    time("read: the first 50 rows only (one screen)", &mut times, || {
        let ids = window.row_ids();
        ids.iter()
            .take(50)
            .filter_map(|id| window.cells().get(id))
            .map(|row| match row {
                loro::ValueOrContainer::Container(loro::Container::Map(map)) => map.get_deep_value(),
                _ => LoroValue::Null,
            })
            .count()
    });
    let rows = time("read every cell", &mut times, || read(&loaded));
    assert_eq!(rows.len(), ROWS);

    let price = 2;
    let status = 3;
    let sorted = time("sort by a number", &mut times, || {
        let mut order: Vec<&Row> = rows.iter().collect();
        order.sort_by(|a, b| {
            number(a.values[price].as_ref())
                .total_cmp(&number(b.values[price].as_ref()))
                .then_with(|| a.id.cmp(&b.id))
        });
        order.len()
    });
    assert_eq!(sorted, ROWS);
    let open = time("filter by a select", &mut times, || {
        rows.iter()
            .filter(|r| r.values[status] == Some(LoroValue::from("o-1")))
            .count()
    });
    assert!(open > 0);

    let before = loaded.doc.oplog_vv();
    time("edit one cell", &mut times, || -> LoroResult<()> {
        loaded.set("row-04321", "c-price", 1.0)
    })?;
    let update = export(&loaded.doc, ExportMode::updates(&before))?;

    // The alternative: every row's properties in its own document.
    let one_row = LoroDoc::new();
    fill(&one_row.get_map("properties"), 7)?;
    one_row.commit();
    let row_snapshot = export(&one_row, ExportMode::Snapshot)?;
    time(
        "alternative: load a document per row",
        &mut times,
        || -> LoroResult<()> {
            for _ in 0..ROWS {
                let doc = LoroDoc::new();
                doc.import(&row_snapshot)?;
            }
            Ok(())
        },
    )?;

    println!("\n| {ROWS} rows × {} columns | |", COLUMNS.len());
    println!("|---|---|");
    println!("| snapshot size | {} KB |", snapshot.len() / 1024);
    println!("| one-cell update | {} bytes |", update.len());
    println!(
        "| alternative: snapshots, a document per row | {} KB |",
        row_snapshot.len() * ROWS / 1024
    );
    for (label, took) in times {
        println!("| {label} | {:.1} ms |", took.as_secs_f64() * 1000.0);
    }
    Ok(())
}
