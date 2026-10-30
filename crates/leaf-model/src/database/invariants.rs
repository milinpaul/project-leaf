//! Checks a database document against the invariants in `docs/architecture/BLOCK_MODEL.md` "Database doc".
//!
//! Reads the raw Loro containers rather than the read API or the index, so a bug in either can't hide a
//! broken document. Used after every command in the property tests.

use std::collections::HashSet;

use leaf_core::{ColumnId, OptionId, PageId};
use loro::{Container, LoroTree, LoroValue, ValueOrContainer};

use super::property::keys;
use super::{Database, ID, containers};
use crate::InvariantViolation;
use crate::page_meta::value;

impl Database {
    /// Every state invariant of the database (`BLOCK_MODEL.md` "Database doc", D1–D4), or what is broken.
    pub fn check_invariants(&self) -> Result<(), Vec<InvariantViolation>> {
        let mut broken = Vec::new();
        ids::<ColumnId>(&self.columns, "column", &mut broken);
        ids::<PageId>(&self.rows, "row", &mut broken);
        let schema = self.doc.get_map(containers::SCHEMA);
        let mut columns = Vec::new();
        schema.for_each(|key, entry| columns.push((key.to_owned(), entry)));
        for (key, entry) in columns {
            let node = format!("schema {key}");
            // D3: every schema entry is a column ID with a map holding a type name.
            let column = key
                .parse::<ColumnId>()
                .map_or_else(|_| key.clone(), |c| c.to_string());
            if key.parse::<ColumnId>().is_err() {
                broken.push(violation(3, &node, "schema key is not a column ID".into()));
            }
            let ValueOrContainer::Container(Container::Map(map)) = entry else {
                broken.push(violation(
                    3,
                    &node,
                    format!("column {column}'s schema is not a map"),
                ));
                continue;
            };
            if !matches!(value(&map, keys::TYPE), Some(LoroValue::String(_))) {
                broken.push(violation(3, &node, format!("column {column} has no type name")));
            }
            // D4: option IDs are valid and unique within their column.
            if let Some(ValueOrContainer::Container(Container::MovableList(list))) = map.get(keys::OPTIONS) {
                let mut seen = HashSet::new();
                for item in 0..list.len() {
                    let Some(ValueOrContainer::Container(Container::Map(option))) = list.get(item) else {
                        broken.push(violation(
                            4,
                            &node,
                            format!("column {column}: option {item} is not a map"),
                        ));
                        continue;
                    };
                    let id = match value(&option, keys::ID) {
                        Some(LoroValue::String(text)) => text.parse::<OptionId>().ok(),
                        _ => None,
                    };
                    match id {
                        Some(id) if !seen.insert(id) => {
                            broken.push(violation(4, &node, format!("column {column}: option {id} twice")));
                        }
                        Some(_) => {}
                        None => broken.push(violation(
                            4,
                            &node,
                            format!("column {column}: option {item} has no ID"),
                        )),
                    }
                }
            }
        }
        if broken.is_empty() { Ok(()) } else { Err(broken) }
    }
}

fn violation(invariant: u8, node: &str, detail: String) -> InvariantViolation {
    InvariantViolation {
        invariant,
        node: node.to_owned(),
        detail,
    }
}

/// D1: every node of a column or row tree has a valid ID (a merge may leave one ID on two nodes; readers use
/// the first). D2: the trees are flat.
fn ids<K: std::str::FromStr>(tree: &LoroTree, part: &str, broken: &mut Vec<InvariantViolation>) {
    for node in tree.get_nodes(false) {
        let name = format!("{:?}", node.id);
        if !matches!(node.parent, loro::TreeParentId::Root) {
            broken.push(violation(2, &name, format!("{part} is nested")));
        }
        let id = match tree.get_meta(node.id).ok().and_then(|meta| value(&meta, ID)) {
            Some(LoroValue::String(text)) => text.parse::<K>().ok(),
            _ => None,
        };
        if id.is_none() {
            broken.push(violation(1, &name, format!("{part} has no valid ID")));
        }
    }
}
