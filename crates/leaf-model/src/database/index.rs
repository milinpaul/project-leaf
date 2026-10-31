//! Two-way maps between a database's permanent column and row IDs and Loro's current `TreeID`s.
//!
//! Rebuilt from the IDs in node meta on load, after undo and redo, and after merging, because those can
//! re-create nodes under new `TreeID`s (ADR-006). A merge can leave one ID on two nodes; the first in order
//! counts, and [`live`] reads each ID once.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::str::FromStr;

use leaf_core::{ColumnId, PageId};
use loro::{LoroTree, LoroValue, TreeID};

use super::ID;
use crate::ModelError;
use crate::page_meta::value;

/// Two-way maps between permanent IDs and Loro's current `TreeID`s, for both trees.
#[derive(Default)]
pub(super) struct Index {
    pub(super) columns: IdMap<ColumnId>,
    pub(super) rows: IdMap<PageId>,
}

pub(super) struct IdMap<K> {
    pub(super) by_id: HashMap<K, TreeID>,
    pub(super) by_node: HashMap<TreeID, K>,
}

impl<K> Default for IdMap<K> {
    fn default() -> Self {
        Self {
            by_id: HashMap::new(),
            by_node: HashMap::new(),
        }
    }
}

impl Index {
    pub(super) fn build(columns: &LoroTree, rows: &LoroTree) -> Result<Self, ModelError> {
        Ok(Self {
            columns: IdMap::build(columns, "column")?,
            rows: IdMap::build(rows, "row")?,
        })
    }
}

impl<K: Copy + Eq + Hash + FromStr> IdMap<K> {
    /// Reads every node's `id`, top-level nodes first in order. Fails on a missing or unreadable one. An ID on
    /// two nodes (a merge can leave that) maps to the first.
    fn build(tree: &LoroTree, part: &'static str) -> Result<Self, ModelError> {
        let mut map = Self::default();
        let roots = tree.roots();
        let rest = tree
            .get_nodes(false)
            .into_iter()
            .map(|n| n.id)
            .filter(|id| !roots.contains(id));
        for node in roots.iter().copied().chain(rest) {
            let invalid = |reason: &str| ModelError::InvalidDatabaseNode {
                part,
                node: format!("{node:?}"),
                reason: reason.to_owned(),
            };
            let id = match value(&tree.get_meta(node)?, ID) {
                Some(LoroValue::String(text)) => text.parse::<K>().map_err(|_| invalid("id is not valid"))?,
                Some(_) => return Err(invalid("id is not text")),
                None => return Err(invalid("id is missing")),
            };
            map.by_id.entry(id).or_insert(node);
            map.by_node.insert(node, id);
        }
        Ok(map)
    }

    pub(super) fn insert(&mut self, id: K, node: TreeID) {
        self.by_id.insert(id, node);
        self.by_node.insert(node, id);
    }
}

/// The tree's top-level nodes in order that `alive` keeps, each ID once, with the node that places it.
pub(super) fn live<K: Copy + Eq + Hash>(
    tree: &LoroTree,
    map: &IdMap<K>,
    alive: impl Fn(K) -> bool,
) -> Vec<(K, TreeID)> {
    let mut seen = HashSet::new();
    tree.roots()
        .into_iter()
        .filter_map(|node| map.by_node.get(&node).map(|&id| (id, node)))
        .filter(|&(id, node)| map.by_id.get(&id) == Some(&node) && seen.insert(id) && alive(id))
        .collect()
}
