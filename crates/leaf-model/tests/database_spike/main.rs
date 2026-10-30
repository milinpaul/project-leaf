//! E09-S01 spike: proves a data model for databases (Phase 1) on Loro before `leaf-model` wraps it.
//!
//! Throwaway quality on purpose, like `loro_spike.rs` for blocks: these tests drive Loro directly and pin down
//! how a database document behaves. Findings and the proposed model are in ADR 0066.
//!
//! The model under test, one Loro document per database:
//!
//! ```text
//! LoroDoc (database)
//! ├── "columns": LoroMovableList<String>   column IDs in display order
//! ├── "schema":  LoroMap                   column ID -> LoroMap { "name", "type", "options", ... }
//! ├── "rows":    LoroMovableList<String>   row page IDs in manual order
//! └── "cells":   LoroMap                   row ID -> LoroMap (mergeable) column ID -> value
//! ```
//!
//! A cell's value is a plain Loro value (last writer wins per cell), except multi-select, which is a
//! mergeable map of option ID -> true so two people adding tags both keep theirs. Cells whose row or
//! column is gone are left in place and ignored when reading, so undo and late edits never lose data.

mod merges;
mod perf;

use loro::{ExportMode, LoroDoc, LoroError, LoroMap, LoroMovableList, LoroResult, LoroValue, UndoManager};

/// One database document, as a peer holds it.
pub(crate) struct Db {
    pub(crate) doc: LoroDoc,
}

impl Db {
    pub(crate) fn new(peer: u64) -> LoroResult<Self> {
        let doc = LoroDoc::new();
        doc.set_peer_id(peer)?;
        Ok(Self { doc })
    }

    /// A second peer starting from this one's state.
    pub(crate) fn fork(&self, peer: u64) -> LoroResult<Self> {
        let other = Self::new(peer)?;
        other.doc.import(&export(&self.doc, ExportMode::all_updates())?)?;
        Ok(other)
    }

    pub(crate) fn columns(&self) -> LoroMovableList {
        self.doc.get_movable_list("columns")
    }

    pub(crate) fn schema(&self) -> LoroMap {
        self.doc.get_map("schema")
    }

    pub(crate) fn rows(&self) -> LoroMovableList {
        self.doc.get_movable_list("rows")
    }

    pub(crate) fn cells(&self) -> LoroMap {
        self.doc.get_map("cells")
    }

    /// Adds a column at the end. Column IDs are fresh UUIDs in the real model, so two peers never create the
    /// same one and its schema map needn't be mergeable.
    pub(crate) fn add_column(&self, id: &str, name: &str, kind: &str) -> LoroResult<LoroMap> {
        let column = self.schema().insert_container(id, LoroMap::new())?;
        column.insert("name", name)?;
        column.insert("type", kind)?;
        self.columns().push(id)?;
        self.doc.commit();
        Ok(column)
    }

    /// Deletes a column from the order and the schema. Its cells stay (ignored when reading).
    pub(crate) fn delete_column(&self, id: &str) -> LoroResult<()> {
        if let Some(at) = position(&self.columns().get_value(), id) {
            self.columns().delete(at, 1)?;
        }
        self.schema().delete(id)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn add_row(&self, id: &str) -> LoroResult<()> {
        self.rows().push(id)?;
        self.doc.commit();
        Ok(())
    }

    /// A row's cells. Mergeable: two peers editing a row for the first time at once share one map rather
    /// than one of them losing every cell they set.
    pub(crate) fn row(&self, id: &str) -> LoroResult<LoroMap> {
        self.cells().ensure_mergeable_map(id)
    }

    pub(crate) fn set(&self, row: &str, column: &str, value: impl Into<LoroValue>) -> LoroResult<()> {
        self.row(row)?.insert(column, value)?;
        self.doc.commit();
        Ok(())
    }

    /// Adds an option to a multi-select cell.
    pub(crate) fn tag(&self, row: &str, column: &str, option: &str) -> LoroResult<()> {
        self.row(row)?
            .ensure_mergeable_map(column)?
            .insert(option, true)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn untag(&self, row: &str, column: &str, option: &str) -> LoroResult<()> {
        self.row(row)?.ensure_mergeable_map(column)?.delete(option)?;
        self.doc.commit();
        Ok(())
    }

    /// A raw cell value, whether or not its row and column still exist.
    pub(crate) fn raw(&self, row: &str, column: &str) -> Option<LoroValue> {
        at(&self.cells().get_deep_value(), &[row, column])
    }

    pub(crate) fn column_ids(&self) -> Vec<String> {
        strings(&self.columns().get_value())
    }

    pub(crate) fn row_ids(&self) -> Vec<String> {
        strings(&self.rows().get_value())
    }

    /// What a table view shows: a header of column names, then each row's cells as text, in order. Cells
    /// of deleted rows or columns don't show.
    pub(crate) fn table(&self) -> Vec<Vec<String>> {
        let schema = self.schema().get_deep_value();
        let cells = self.cells().get_deep_value();
        let columns = self.column_ids();
        let mut out = vec![
            columns
                .iter()
                .map(|c| at(&schema, &[c, "name"]).map_or_else(|| "?".into(), |v| show(&v)))
                .collect(),
        ];
        for row in self.row_ids() {
            out.push(
                columns
                    .iter()
                    .map(|c| at(&cells, &[&row, c]).map_or_else(String::new, |v| show(&v)))
                    .collect(),
            );
        }
        out
    }
}

/// Sends each peer what the other has, both ways.
pub(crate) fn sync(a: &Db, b: &Db) -> LoroResult<()> {
    let to_b = export(&a.doc, ExportMode::updates(&b.doc.oplog_vv()))?;
    let to_a = export(&b.doc, ExportMode::updates(&a.doc.oplog_vv()))?;
    b.doc.import(&to_b)?;
    a.doc.import(&to_a)?;
    Ok(())
}

pub(crate) fn export(doc: &LoroDoc, mode: ExportMode) -> LoroResult<Vec<u8>> {
    doc.export(mode)
        .map_err(|e| LoroError::ArgErr(e.to_string().into()))
}

pub(crate) fn undo_manager(db: &Db) -> UndoManager {
    let mut undo = UndoManager::new(&db.doc);
    undo.set_merge_interval(0);
    undo
}

/// The value at `path` inside nested maps.
pub(crate) fn at(value: &LoroValue, path: &[&str]) -> Option<LoroValue> {
    let mut current = value.clone();
    for key in path {
        let LoroValue::Map(map) = current else {
            return None;
        };
        current = map.get(*key)?.clone();
    }
    Some(current)
}

fn position(list: &LoroValue, id: &str) -> Option<usize> {
    strings(list).iter().position(|s| s == id)
}

fn strings(list: &LoroValue) -> Vec<String> {
    match list {
        LoroValue::List(items) => items
            .iter()
            .filter_map(|v| match v {
                LoroValue::String(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A cell value as a table shows it: a multi-select's options sorted and comma-separated.
pub(crate) fn show(value: &LoroValue) -> String {
    match value {
        LoroValue::String(s) => s.to_string(),
        LoroValue::Double(d) => d.to_string(),
        LoroValue::I64(i) => i.to_string(),
        LoroValue::Bool(b) => b.to_string(),
        LoroValue::Map(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            keys.into_iter().map(String::as_str).collect::<Vec<_>>().join(",")
        }
        other => format!("{other:?}"),
    }
}

#[test]
fn schema_and_cells_live_in_one_document_and_survive_a_save() -> LoroResult<()> {
    let db = Db::new(1)?;
    let status = db.add_column("c-status", "Status", "select")?;
    let options = status.insert_container("options", loro::LoroMovableList::new())?;
    for (id, name) in [("o-todo", "To do"), ("o-done", "Done")] {
        let option = options.push_container(LoroMap::new())?;
        option.insert("id", id)?;
        option.insert("name", name)?;
    }
    db.add_column("c-price", "Price", "number")?;
    db.add_column("c-ok", "Checked", "checkbox")?;
    db.add_row("row-a")?;
    db.add_row("row-b")?;
    db.set("row-a", "c-status", "o-done")?;
    db.set("row-a", "c-price", 2.5)?;
    db.set("row-b", "c-ok", true)?;
    // Moving a column is one move in the order list; the schema and cells don't change.
    db.columns().mov(2, 0)?;
    db.doc.commit();
    let expected = vec![
        vec!["Checked", "Status", "Price"],
        vec!["", "o-done", "2.5"],
        vec!["true", "", ""],
    ];
    assert_eq!(db.table(), expected);

    let saved = export(&db.doc, ExportMode::Snapshot)?;
    let loaded = Db::new(2)?;
    loaded.doc.import(&saved)?;
    assert_eq!(loaded.table(), expected);
    assert_eq!(
        at(&loaded.schema().get_deep_value(), &["c-status", "options"])
            .map(|v| matches!(v, LoroValue::List(l) if l.len() == 2)),
        Some(true)
    );
    Ok(())
}

#[test]
fn a_property_type_from_a_newer_version_survives_an_older_peers_edits() -> LoroResult<()> {
    let newer = Db::new(1)?;
    newer.add_column("c-name", "Name", "text")?;
    let formula = newer.add_column("c-total", "Total", "formula")?;
    formula.insert("expression", "prop(\"Price\") * 2")?;
    let format = formula.insert_container("format", LoroMap::new())?;
    format.insert("currency", "EUR")?;
    newer.add_row("row-a")?;
    let cached = newer.row("row-a")?.insert_container("c-total", LoroMap::new())?;
    cached.insert("cached", 42.0)?;
    newer.doc.commit();

    // The older peer knows nothing about formulas. It writes only keys it knows: it renames the column,
    // moves it, and edits another cell in the same row.
    let older = newer.fork(2)?;
    let schema = older.schema().get_deep_value();
    assert_eq!(
        at(&schema, &["c-total", "type"]).map(|v| show(&v)),
        Some("formula".into()),
        "read as an unknown type and shown read-only"
    );
    if let Some(loro::ValueOrContainer::Container(loro::Container::Map(column))) =
        older.schema().get("c-total")
    {
        column.insert("name", "Total (EUR)")?;
    }
    older.columns().mov(1, 0)?;
    older.set("row-a", "c-name", "Lamp")?;
    sync(&newer, &older)?;

    for db in [&newer, &older] {
        let schema = db.schema().get_deep_value();
        assert_eq!(
            at(&schema, &["c-total", "expression"]).map(|v| show(&v)),
            Some("prop(\"Price\") * 2".into())
        );
        assert_eq!(
            at(&schema, &["c-total", "format", "currency"]).map(|v| show(&v)),
            Some("EUR".into())
        );
        assert_eq!(
            db.raw("row-a", "c-total").and_then(|v| at(&v, &["cached"])),
            Some(LoroValue::Double(42.0))
        );
        assert_eq!(db.table()[0], ["Total (EUR)", "Name"]);
        assert_eq!(db.raw("row-a", "c-name"), Some("Lamp".into()));
    }
    Ok(())
}

#[test]
fn undo_restores_a_cell_and_a_deleted_column_exactly() -> LoroResult<()> {
    let db = Db::new(1)?;
    let mut undo = undo_manager(&db);
    let status = db.add_column("c-status", "Status", "select")?;
    status.insert("width", 180.0)?;
    db.doc.commit();
    db.add_column("c-price", "Price", "number")?;
    db.add_row("row-a")?;
    db.set("row-a", "c-price", 1.0)?;
    db.set("row-a", "c-status", "o-done")?;
    let before = (db.table(), db.schema().get_deep_value());

    db.set("row-a", "c-price", 9.0)?;
    assert!(undo.undo()?);
    assert_eq!(db.raw("row-a", "c-price"), Some(LoroValue::Double(1.0)));

    db.delete_column("c-status")?;
    assert_eq!(db.table()[0], ["Price"]);
    assert_eq!(
        db.raw("row-a", "c-status"),
        Some("o-done".into()),
        "the cells of a deleted column stay"
    );
    assert!(undo.undo()?);
    assert_eq!((db.table(), db.schema().get_deep_value()), before);
    assert!(undo.redo()?);
    assert_eq!(db.table()[0], ["Price"]);
    Ok(())
}
