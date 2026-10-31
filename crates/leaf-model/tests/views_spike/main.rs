//! E22-S01 spike: proves a data model for saved database views and a board layout on Loro, before `leaf-model`
//! grows them. Throwaway quality on purpose, like `database_spike`: these tests drive Loro directly. Findings and the
//! proposed model are in ADR 0075.
//!
//! The database document as it is today (ADR 0066), plus two new containers:
//!
//! ```text
//! LoroDoc (database)
//! ├── "columns":       LoroTree (flat)  column order (unchanged)
//! ├── "schema":        LoroMap          column ID -> { name, type, hidden, width, options } (unchanged)
//! ├── "rows":          LoroTree (flat)  row order; meta "id", "deleted" (unchanged)
//! ├── "cells":         LoroMap          row ID -> mergeable map: column ID -> value (unchanged)
//! ├── "view":          LoroMap          the first view, ID "default": sort, filters (+ name, layout, group_by)
//! ├── "views":         LoroTree (flat)  NEW: tab order; meta "id" = view ID. Nodes are never deleted.
//! └── "view_settings": LoroMap          NEW: view ID -> { name, layout, sort, filters, group_by, hidden, widths,
//!                                       hidden_groups, ...unknown keys kept }. A view exists while its entry does.
//! ```
//!
//! The first view stays where this version keeps it, so a database saved today opens unchanged and a peer that
//! knows only one table view keeps editing it: its sort and filters in `"view"`, its hidden columns and widths in
//! the schema. Every other view keeps all of its settings in its own map.

mod merges;
mod perf;

use loro::{
    ExportMode, LoroDoc, LoroError, LoroList, LoroMap, LoroResult, LoroTree, LoroValue, TreeID, TreeParentId,
    UndoManager,
};

/// The first view's ID: the database's own table view, as saved before views existed.
pub(crate) const DEFAULT: &str = "default";

/// One database document, as a peer holds it.
pub(crate) struct Db {
    pub(crate) doc: LoroDoc,
}

/// A view as the UI would read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct View {
    pub(crate) id: String,
    pub(crate) name: String,
    /// `"table"`, `"board"`, or a newer version's layout, kept as named.
    pub(crate) layout: String,
    /// The column a board groups by.
    pub(crate) group_by: Option<String>,
    /// Column IDs the view hides.
    pub(crate) hidden: Vec<String>,
    /// The column it sorts by, if any (the real model stores a list; one is enough here).
    pub(crate) sort: Option<String>,
}

/// One group of a board: the option it stands for (`None` for "No value") and its cards, in order.
pub(crate) type Group = (Option<String>, Vec<String>);

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

    fn tree(&self, name: &str) -> LoroTree {
        let tree = self.doc.get_tree(name);
        tree.enable_fractional_index(0);
        tree
    }

    pub(crate) fn schema(&self) -> LoroMap {
        self.doc.get_map("schema")
    }

    pub(crate) fn cells(&self) -> LoroMap {
        self.doc.get_map("cells")
    }

    /// The first view's settings, as saved before views existed.
    pub(crate) fn legacy_view(&self) -> LoroMap {
        self.doc.get_map("view")
    }

    pub(crate) fn settings(&self) -> LoroMap {
        self.doc.get_map("view_settings")
    }

    // ---- columns and rows, as today ----

    /// Adds a select column with `options` (IDs; each named after its ID).
    pub(crate) fn add_select(&self, id: &str, options: &[&str]) -> LoroResult<()> {
        let column = self.schema().insert_container(id, LoroMap::new())?;
        column.insert("name", id)?;
        column.insert("type", "select")?;
        let list = column.insert_container("options", LoroList::new())?;
        for option in options {
            let map = list.push_container(LoroMap::new())?;
            map.insert("id", *option)?;
            map.insert("name", *option)?;
        }
        let node = self.tree("columns").create(TreeParentId::Root)?;
        self.tree("columns").get_meta(node)?.insert("id", id)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn add_text(&self, id: &str) -> LoroResult<()> {
        let column = self.schema().insert_container(id, LoroMap::new())?;
        column.insert("name", id)?;
        column.insert("type", "text")?;
        let node = self.tree("columns").create(TreeParentId::Root)?;
        self.tree("columns").get_meta(node)?.insert("id", id)?;
        self.doc.commit();
        Ok(())
    }

    /// The option IDs of `column`, in order.
    pub(crate) fn options(&self, column: &str) -> Vec<String> {
        match at(&self.schema().get_deep_value(), &[column, "options"]) {
            Some(LoroValue::List(items)) => items
                .iter()
                .filter_map(|o| match at(o, &["id"]) {
                    Some(LoroValue::String(s)) => Some(s.to_string()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn delete_option(&self, column: &str, option: &str) -> LoroResult<()> {
        let Some(at_index) = self.options(column).iter().position(|o| o == option) else {
            return Ok(());
        };
        let Some(loro::ValueOrContainer::Container(loro::Container::Map(column))) = self.schema().get(column)
        else {
            return Ok(());
        };
        let Some(loro::ValueOrContainer::Container(loro::Container::List(list))) = column.get("options")
        else {
            return Ok(());
        };
        list.delete(at_index, 1)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn delete_column(&self, id: &str) -> LoroResult<()> {
        self.schema().delete(id)?;
        self.doc.commit();
        Ok(())
    }

    /// Hides `column` the way this version does: in its schema, for the one table view.
    pub(crate) fn hide_in_schema(&self, column: &str) -> LoroResult<()> {
        if let Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) = self.schema().get(column)
        {
            map.insert("hidden", true)?;
        }
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn add_row(&self, id: &str) -> LoroResult<()> {
        let tree = self.tree("rows");
        let node = tree.create(TreeParentId::Root)?;
        tree.get_meta(node)?.insert("id", id)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn delete_row(&self, id: &str) -> LoroResult<()> {
        if let Some(node) = node_of(&self.tree("rows"), id) {
            self.tree("rows").get_meta(node)?.insert("deleted", true)?;
        }
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn set(&self, row: &str, column: &str, value: impl Into<LoroValue>) -> LoroResult<()> {
        self.cells().ensure_mergeable_map(row)?.insert(column, value)?;
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn cell(&self, row: &str, column: &str) -> Option<LoroValue> {
        at(&self.cells().get_deep_value(), &[row, column])
    }

    /// Live rows in order: deleted ones left out, a node a merge left twice read once.
    pub(crate) fn rows(&self) -> Vec<String> {
        ids(&self.tree("rows"), |meta| {
            !matches!(meta.get("deleted"), Some(LoroValue::Bool(true)))
        })
    }

    // ---- views ----

    /// The tabs, in order. The first view is always there, first unless moved, even before it has a node.
    pub(crate) fn view_ids(&self) -> Vec<String> {
        let settings = self.settings().get_deep_value();
        let mut tabs: Vec<String> = ids(&self.tree("views"), |_| true)
            .into_iter()
            .filter(|id| id == DEFAULT || at(&settings, &[id]).is_some())
            .collect();
        if !tabs.iter().any(|id| id == DEFAULT) {
            tabs.insert(0, DEFAULT.to_owned());
        }
        tabs
    }

    /// The first view's node, made the first time the tabs are reordered or added to. Two peers can both make it;
    /// reading takes the first.
    fn ensure_default_node(&self) -> LoroResult<()> {
        let tree = self.tree("views");
        if node_of(&tree, DEFAULT).is_none() {
            let node = tree.create_at(TreeParentId::Root, 0)?;
            tree.get_meta(node)?.insert("id", DEFAULT)?;
        }
        Ok(())
    }

    /// Adds a view at the end of the tabs.
    pub(crate) fn add_view(
        &self,
        id: &str,
        name: &str,
        layout: &str,
        group_by: Option<&str>,
    ) -> LoroResult<()> {
        self.ensure_default_node()?;
        let map = self.settings().insert_container(id, LoroMap::new())?;
        map.insert("name", name)?;
        map.insert("layout", layout)?;
        if let Some(column) = group_by {
            map.insert("group_by", column)?;
        }
        map.insert_container("hidden", LoroMap::new())?;
        let tree = self.tree("views");
        let node = tree.create(TreeParentId::Root)?;
        tree.get_meta(node)?.insert("id", id)?;
        self.doc.commit();
        Ok(())
    }

    /// The map a view's settings live in: `"view"` for the first view, its own map otherwise.
    fn view_map(&self, id: &str) -> Option<LoroMap> {
        if id == DEFAULT {
            return Some(self.legacy_view());
        }
        match self.settings().get(id) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(map))) => Some(map),
            _ => None,
        }
    }

    pub(crate) fn set_view(&self, id: &str, key: &str, value: impl Into<LoroValue>) -> LoroResult<()> {
        if let Some(map) = self.view_map(id) {
            map.insert(key, value)?;
        }
        self.doc.commit();
        Ok(())
    }

    /// Hides `column` in a view. The first view hides in the schema, as this version does.
    pub(crate) fn hide(&self, view: &str, column: &str) -> LoroResult<()> {
        if view == DEFAULT {
            return self.hide_in_schema(column);
        }
        if let Some(map) = self.view_map(view) {
            let hidden = match map.get("hidden") {
                Some(loro::ValueOrContainer::Container(loro::Container::Map(hidden))) => hidden,
                _ => map.insert_container("hidden", LoroMap::new())?,
            };
            hidden.insert(column, true)?;
        }
        self.doc.commit();
        Ok(())
    }

    /// Deletes a view: its settings go, its node stays (a move on another device can't bring it back).
    pub(crate) fn delete_view(&self, id: &str) -> LoroResult<()> {
        assert_ne!(id, DEFAULT, "the first view stays (see ADR 0075 for why)");
        self.settings().delete(id)?;
        self.doc.commit();
        Ok(())
    }

    /// Moves tab `id` to just before tab `before`.
    pub(crate) fn move_view(&self, id: &str, before: &str) -> LoroResult<()> {
        self.ensure_default_node()?;
        let tree = self.tree("views");
        if let (Some(node), Some(target)) = (node_of(&tree, id), node_of(&tree, before)) {
            tree.mov_before(node, target)?;
        }
        self.doc.commit();
        Ok(())
    }

    pub(crate) fn view(&self, id: &str) -> Option<View> {
        let map = self.view_map(id)?.get_deep_value();
        let text = |key: &str| match at(&map, &[key]) {
            Some(LoroValue::String(s)) => Some(s.to_string()),
            _ => None,
        };
        let hidden = if id == DEFAULT {
            let schema = self.schema().get_deep_value();
            match &schema {
                LoroValue::Map(columns) => {
                    let mut hidden: Vec<String> = columns
                        .iter()
                        .filter(|(_, c)| matches!(at(c, &["hidden"]), Some(LoroValue::Bool(true))))
                        .map(|(id, _)| id.clone())
                        .collect();
                    hidden.sort();
                    hidden
                }
                _ => Vec::new(),
            }
        } else {
            match at(&map, &["hidden"]) {
                Some(LoroValue::Map(columns)) => {
                    let mut hidden: Vec<String> = columns.keys().cloned().collect();
                    hidden.sort();
                    hidden
                }
                _ => Vec::new(),
            }
        };
        Some(View {
            id: id.to_owned(),
            name: text("name").unwrap_or_else(|| "Table".to_owned()),
            layout: text("layout").unwrap_or_else(|| "table".to_owned()),
            group_by: text("group_by"),
            hidden,
            sort: text("sort"),
        })
    }

    // ---- board ----

    /// The board of `view`: "No value" first, then one group per option of its grouping column, in option order;
    /// cards in row order. A cell naming an option that no longer exists counts as no value. `None` when the view
    /// has no grouping column, or that column is gone.
    pub(crate) fn board(&self, view: &str) -> Option<Vec<Group>> {
        let column = self.view(view)?.group_by?;
        at(&self.schema().get_deep_value(), &[&column])?;
        let options = self.options(&column);
        let mut groups: Vec<Group> = std::iter::once(None)
            .chain(options.iter().cloned().map(Some))
            .map(|o| (o, Vec::new()))
            .collect();
        let cells = self.cells().get_deep_value();
        for row in self.rows() {
            let value = match at(&cells, &[&row, &column]) {
                Some(LoroValue::String(option)) if options.contains(&option) => Some(option.to_string()),
                _ => None,
            };
            if let Some(group) = groups.iter_mut().find(|(o, _)| *o == value) {
                group.1.push(row);
            }
        }
        Some(groups)
    }

    /// Drags card `row` into the group for `option` (`None`: no value), just before card `before` (`None`: at the
    /// end). The cell and the row's place change together.
    pub(crate) fn move_card(
        &self,
        row: &str,
        column: &str,
        option: Option<&str>,
        before: Option<&str>,
    ) -> LoroResult<()> {
        let cells = self.cells().ensure_mergeable_map(row)?;
        match option {
            Some(option) => cells.insert(column, option)?,
            None => cells.delete(column)?,
        }
        let tree = self.tree("rows");
        if let Some(node) = node_of(&tree, row) {
            match before.and_then(|b| node_of(&tree, b)) {
                Some(target) => tree.mov_before(node, target)?,
                None => {
                    if let Some(last) = tree.children(TreeParentId::Root).and_then(|c| c.last().copied())
                        && last != node
                    {
                        tree.mov_after(node, last)?;
                    }
                }
            }
        }
        self.doc.commit();
        Ok(())
    }
}

/// The IDs of a flat tree's nodes in order, keeping those whose meta passes `keep`, each ID once.
fn ids(tree: &LoroTree, keep: impl Fn(&std::collections::HashMap<String, LoroValue>) -> bool) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    tree.children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|node| {
            let meta = tree.get_meta(node).ok()?.get_deep_value();
            let LoroValue::Map(meta) = meta else {
                return None;
            };
            let meta: std::collections::HashMap<String, LoroValue> =
                meta.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let Some(LoroValue::String(id)) = meta.get("id") else {
                return None;
            };
            let id = id.to_string();
            (keep(&meta) && seen.insert(id.clone())).then_some(id)
        })
        .collect()
}

fn node_of(tree: &LoroTree, id: &str) -> Option<TreeID> {
    tree.children(TreeParentId::Root)?.into_iter().find(|&node| {
        tree.get_meta(node)
            .ok()
            .and_then(|meta| meta.get("id"))
            .is_some_and(
                |v| matches!(v, loro::ValueOrContainer::Value(LoroValue::String(s)) if s.as_str() == id),
            )
    })
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

/// A database with a "status" select (todo, doing, done), a "notes" text column and rows r1..r5.
pub(crate) fn kanban(peer: u64) -> LoroResult<Db> {
    let db = Db::new(peer)?;
    db.add_select("status", &["todo", "doing", "done"])?;
    db.add_text("notes")?;
    for row in ["r1", "r2", "r3", "r4", "r5"] {
        db.add_row(row)?;
    }
    db.set("r1", "status", "todo")?;
    db.set("r2", "status", "done")?;
    db.set("r3", "status", "todo")?;
    db.set("r5", "status", "doing")?;
    Ok(db)
}

fn groups(board: &[Group]) -> Vec<(Option<&str>, Vec<&str>)> {
    board
        .iter()
        .map(|(o, rows)| (o.as_deref(), rows.iter().map(String::as_str).collect()))
        .collect()
}

#[test]
fn a_database_saved_today_opens_with_its_one_table_view_and_reading_writes_nothing() -> LoroResult<()> {
    let db = kanban(1)?;
    db.legacy_view().insert("sort", "notes")?;
    db.hide_in_schema("notes")?;
    let before = db.doc.oplog_vv();

    assert_eq!(db.view_ids(), [DEFAULT]);
    let view = db.view(DEFAULT).expect("the first view");
    assert_eq!((view.name.as_str(), view.layout.as_str()), ("Table", "table"));
    assert_eq!(view.sort.as_deref(), Some("notes"));
    assert_eq!(view.hidden, ["notes"], "hidden where this version keeps it");
    assert_eq!(db.doc.oplog_vv(), before, "opening and reading change nothing");
    Ok(())
}

#[test]
fn each_view_keeps_its_own_settings_and_tabs_reorder() -> LoroResult<()> {
    let db = kanban(1)?;
    db.add_view("v-board", "By status", "board", Some("status"))?;
    db.add_view("v-notes", "Notes", "table", None)?;
    assert_eq!(db.view_ids(), [DEFAULT, "v-board", "v-notes"]);

    db.hide("v-board", "notes")?;
    db.set_view("v-notes", "sort", "notes")?;
    assert_eq!(
        db.view("v-board").map(|v| v.hidden),
        Some(vec!["notes".to_owned()])
    );
    assert_eq!(
        db.view(DEFAULT).map(|v| v.hidden),
        Some(Vec::new()),
        "the first view still shows notes"
    );
    assert_eq!(db.view(DEFAULT).and_then(|v| v.sort), None);

    db.move_view("v-notes", DEFAULT)?;
    assert_eq!(db.view_ids(), ["v-notes", DEFAULT, "v-board"]);
    db.set_view(DEFAULT, "name", "Everything")?;
    assert_eq!(db.view(DEFAULT).map(|v| v.name), Some("Everything".to_owned()));

    db.delete_view("v-notes")?;
    assert_eq!(db.view_ids(), [DEFAULT, "v-board"]);
    assert!(db.view("v-notes").is_none());
    Ok(())
}

#[test]
fn a_board_groups_rows_by_option_in_option_order_with_no_value_first() -> LoroResult<()> {
    let db = kanban(1)?;
    db.add_view("v-board", "Board", "board", Some("status"))?;
    db.add_row("r6")?;
    db.set("r6", "status", "o-gone")?; // an option that no longer exists
    db.delete_row("r3")?;

    let board = db.board("v-board").expect("grouped by status");
    assert_eq!(
        groups(&board),
        [
            (None, vec!["r4", "r6"]),
            (Some("todo"), vec!["r1"]),
            (Some("doing"), vec!["r5"]),
            (Some("done"), vec!["r2"]),
        ]
    );
    // The table view has no grouping.
    assert!(db.board(DEFAULT).is_none());
    Ok(())
}

#[test]
fn moving_a_card_changes_its_value_and_place_in_one_undo_step() -> LoroResult<()> {
    let db = kanban(1)?;
    db.add_view("v-board", "Board", "board", Some("status"))?;
    let mut undo = UndoManager::new(&db.doc);
    undo.set_merge_interval(0);

    // r3 (todo) dragged into "done", above r2.
    undo.group_start()?;
    db.move_card("r3", "status", Some("done"), Some("r2"))?;
    undo.group_end();
    let after = db.board("v-board").expect("board");
    assert_eq!(
        after[3],
        (Some("done".to_owned()), vec!["r3".to_owned(), "r2".to_owned()])
    );
    assert_eq!(db.rows(), ["r1", "r3", "r2", "r4", "r5"]);

    assert!(undo.undo()?);
    assert_eq!(db.cell("r3", "status"), Some("todo".into()));
    assert_eq!(
        db.rows(),
        ["r1", "r2", "r3", "r4", "r5"],
        "value and place come back together"
    );
    assert!(undo.redo()?);
    assert_eq!(db.cell("r3", "status"), Some("done".into()));
    assert_eq!(db.rows(), ["r1", "r3", "r2", "r4", "r5"]);
    Ok(())
}

#[test]
fn a_view_layout_from_a_newer_version_survives_this_versions_edits() -> LoroResult<()> {
    let newer = kanban(1)?;
    newer.add_view("v-cal", "Calendar", "calendar", None)?;
    newer.set_view("v-cal", "date_by", "due")?;
    let older = newer.fork(2)?;

    // This version renames it and hides a column in it, knowing nothing of calendars.
    older.set_view("v-cal", "name", "Due dates")?;
    older.hide("v-cal", "notes")?;
    sync(&older, &newer)?;

    let map = newer.settings().get_deep_value();
    assert_eq!(at(&map, &["v-cal", "layout"]), Some("calendar".into()));
    assert_eq!(
        at(&map, &["v-cal", "date_by"]),
        Some("due".into()),
        "the newer key is untouched"
    );
    assert_eq!(newer.view("v-cal").map(|v| v.name), Some("Due dates".to_owned()));
    Ok(())
}

#[test]
fn a_board_whose_grouping_column_is_deleted_has_no_groups() -> LoroResult<()> {
    let db = kanban(1)?;
    db.add_view("v-board", "Board", "board", Some("status"))?;
    db.delete_column("status")?;
    assert!(
        db.board("v-board").is_none(),
        "the board asks for a property to group by"
    );
    assert_eq!(
        db.view("v-board").and_then(|v| v.group_by).as_deref(),
        Some("status"),
        "kept for undo"
    );
    Ok(())
}
