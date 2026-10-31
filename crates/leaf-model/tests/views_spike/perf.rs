//! How a board over a big database behaves: grouping 10,000 rows by a select, the first screen of every group, and
//! one card move. A measurement, not a check, so it is ignored by default; run it in a release build:
//!
//! ```text
//! cargo test --release -p leaf-model --test views_spike -- --ignored --nocapture
//! ```
#![allow(clippy::print_stdout, reason = "the spike prints its timings")]

use std::time::{Duration, Instant};

use loro::{LoroResult, LoroValue};

use super::{Db, at};

const ROWS: usize = 10_000;
const OPTIONS: [&str; 5] = ["backlog", "todo", "doing", "review", "done"];
/// Cards a group shows before it scrolls.
const FIRST_SCREEN: usize = 25;

fn build() -> LoroResult<Db> {
    let db = Db::new(1)?;
    db.add_select("status", &OPTIONS)?;
    db.add_text("notes")?;
    db.add_view("v-board", "Board", "board", Some("status"))?;
    for i in 0..ROWS {
        let row = format!("row-{i:05}");
        let tree = db.doc.get_tree("rows");
        let node = tree.create(loro::TreeParentId::Root)?;
        tree.get_meta(node)?.insert("id", row.as_str())?;
        let cells = db.cells().ensure_mergeable_map(&row)?;
        // Every sixth row has no status.
        if i % 6 != 0 {
            cells.insert("status", OPTIONS[(i * 7) % OPTIONS.len()])?;
        }
        cells.insert("notes", format!("Notes about item {i}"))?;
    }
    db.doc.commit();
    Ok(db)
}

fn time<T>(what: &str, f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    let took = start.elapsed();
    println!("{what}: {took:.2?}");
    (out, took)
}

#[test]
#[ignore = "a measurement; run in release with --ignored --nocapture"]
fn grouping_ten_thousand_rows_into_a_board() -> LoroResult<()> {
    let db = build()?;

    // Cold: read the rows and every cell from the document, then group (what opening a board costs today).
    let (board, cold) = time("group 10,000 rows from the document", || db.board("v-board"));
    let board = board.expect("grouped");
    assert_eq!(board.iter().map(|(_, rows)| rows.len()).sum::<usize>(), ROWS);

    // Warm: the editor already holds the cells it has read (DatabaseEditor's cache); grouping is then a pass over
    // one column's values.
    let rows = db.rows();
    let cells = db.cells().get_deep_value();
    let values: Vec<Option<LoroValue>> = rows.iter().map(|r| at(&cells, &[r, "status"])).collect();
    let (_, warm) = time("group 10,000 cached values", || {
        let mut counts = [0_usize; 6];
        for value in &values {
            let ix = match value {
                Some(LoroValue::String(s)) => {
                    OPTIONS.iter().position(|o| *o == s.as_str()).map_or(0, |i| i + 1)
                }
                _ => 0,
            };
            counts[ix] += 1;
        }
        counts
    });

    // First screen: the first cards of each group, the rest read later (as the table does).
    let (_, first) = time("first screen of every group", || {
        board
            .iter()
            .map(|(_, rows)| rows.iter().take(FIRST_SCREEN).count())
            .sum::<usize>()
    });

    // One card move: value and place.
    let (_, moved) = time("move one card", || {
        db.move_card("row-00010", "status", Some("done"), Some("row-00002"))
    });

    println!(
        "groups: {:?}",
        board
            .iter()
            .map(|(o, r)| (o.clone(), r.len()))
            .collect::<Vec<_>>()
    );
    assert!(
        cold < Duration::from_secs(2) && warm < Duration::from_millis(50) && first < Duration::from_millis(5)
    );
    assert!(moved < Duration::from_millis(50));
    Ok(())
}
