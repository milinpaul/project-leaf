//! Checks the page document against the invariants in `docs/architecture/BLOCK_MODEL.md`.
//!
//! Reads the raw Loro tree rather than the read API or the index, so a bug in either can't hide a
//! broken document. Used after every command in the property tests; cheap enough to call in debug
//! builds too. Invariant 7 (undo restores the earlier state) is about history, not a single state,
//! so the undo property test checks it instead (E02-S08).

use std::collections::HashSet;
use std::fmt;

use leaf_core::BlockId;
use loro::{Container, LoroValue, TextDelta, TreeID, TreeParentId, ValueOrContainer};

use super::PageDoc;
use super::page_links::link_problem;
use crate::BlockKind;
use crate::block_kind::keys;
use crate::page_meta::value;

/// One broken rule, with the `BLOCK_MODEL.md` invariant number it breaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantViolation {
    /// The invariant's number in `BLOCK_MODEL.md`.
    pub invariant: u8,
    /// Loro's node ID, for debugging (block IDs may be the broken thing).
    pub node: String,
    /// What is wrong.
    pub detail: String,
}

impl fmt::Display for InvariantViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invariant {} broken at node {}: {}",
            self.invariant, self.node, self.detail
        )
    }
}

impl PageDoc {
    /// Every state invariant of the page (`BLOCK_MODEL.md` 1–6 and 8–10), or the list of what is broken.
    pub fn check_invariants(&self) -> Result<(), Vec<InvariantViolation>> {
        let mut broken = Vec::new();
        let mut report = |invariant: u8, node: TreeID, detail: String| {
            broken.push(InvariantViolation {
                invariant,
                node: format!("{node:?}"),
                detail,
            });
        };
        let nodes: Vec<TreeID> = self.tree.get_nodes(false).into_iter().map(|n| n.id).collect();
        let mut ids = HashSet::with_capacity(nodes.len());

        for &node in &nodes {
            let Ok(meta) = self.tree.get_meta(node) else {
                report(1, node, "has no meta map".into());
                continue;
            };
            // 8: a valid, unique id that the index maps back to this node.
            match value(&meta, keys::ID) {
                Some(LoroValue::String(text)) => match text.parse::<BlockId>() {
                    Ok(id) if !ids.insert(id) => report(8, node, format!("id {id} is used twice")),
                    Ok(id) if self.index.by_block.get(&id) != Some(&node) => {
                        report(8, node, format!("id {id} is not indexed to this node"));
                    }
                    Ok(_) => {}
                    Err(err) => report(8, node, format!("id is invalid: {err}")),
                },
                _ => report(8, node, "has no text id".into()),
            }
            // 1 and 4: a type, and properties that read cleanly (heading level 1–3 among them).
            if !matches!(value(&meta, keys::TYPE), Some(LoroValue::String(_))) {
                report(1, node, "has no text type".into());
                continue;
            }
            let kind = match BlockKind::read(&meta) {
                Ok(kind) => kind,
                Err((field, reason)) => {
                    report(4, node, format!("{field} {reason}"));
                    continue;
                }
            };
            if matches!(kind, BlockKind::Unknown { .. }) {
                continue; // a newer version's rules are unknown here; such blocks are left as they are
            }
            // 2: a text container exactly when the kind has text.
            let text = match meta.get(keys::TEXT) {
                Some(ValueOrContainer::Container(Container::Text(text))) => Some(text),
                _ => None,
            };
            if text.is_some() != kind.has_text() {
                let has = if text.is_some() { "has" } else { "lacks" };
                report(2, node, format!("{} {has} a text container", kind.type_name()));
            }
            // 10: a page link is one placeholder with its page mark; the mark is on nothing else.
            if let Some(problem) = text.as_ref().and_then(link_problem) {
                report(10, node, problem);
            }
            // 3 and 9: what a block may hold, and where rows and cells may sit.
            for (invariant, detail) in self.nesting_problems(node, &kind) {
                report(invariant, node, detail);
            }
            // 5: code text carries no marks.
            if let (BlockKind::Code { .. }, Some(text)) = (&kind, &text) {
                let marked = text.to_delta().into_iter().any(|delta| {
                    matches!(delta, TextDelta::Insert { attributes: Some(attrs), .. } if !attrs.is_empty())
                });
                if marked {
                    report(5, node, "code text has marks".into());
                }
            }
        }

        // 6: every node's chain of parents reaches the top level without repeating.
        for &node in &nodes {
            let mut seen = HashSet::new();
            let mut cursor = Some(node);
            while let Some(current) = cursor {
                if !seen.insert(current) {
                    report(6, node, "its parents form a cycle".into());
                    break;
                }
                cursor = match self.tree.parent(current) {
                    Some(TreeParentId::Node(parent)) => Some(parent),
                    _ => None,
                };
            }
        }
        // 8 again: the index holds exactly the live blocks.
        if self.index.by_block.len() != nodes.len() {
            let detail = format!(
                "the index has {} blocks but the tree has {}",
                self.index.by_block.len(),
                nodes.len()
            );
            broken.push(InvariantViolation {
                invariant: 8,
                node: "-".into(),
                detail,
            });
        }

        if broken.is_empty() { Ok(()) } else { Err(broken) }
    }

    /// Invariant 3 (no children under kinds that can't hold them; tables hold only rows, rows only
    /// cells) and 9 (rows only inside tables, cells only inside rows) for one block.
    fn nesting_problems(&self, node: TreeID, kind: &BlockKind) -> Vec<(u8, String)> {
        let mut problems = Vec::new();
        match kind.fixed_child() {
            Some(allowed) => {
                for child in self.tree.children(node).unwrap_or_default() {
                    if let Some(other) = self.raw_kind(child).filter(|k| *k != allowed) {
                        let detail = format!("{} holds a {}", kind.type_name(), other.type_name());
                        problems.push((3, detail));
                    }
                }
            }
            None if !kind.allows_children() && self.tree.children_num(node).unwrap_or(0) > 0 => {
                problems.push((3, format!("{} has children", kind.type_name())));
            }
            None => {}
        }
        if kind.is_table_part() && !self.in_place(node, kind) {
            problems.push((9, format!("{} is outside its table", kind.type_name())));
        }
        problems
    }

    /// The kind stored at `node`, if it reads; `None` for unknown kinds too, whose rules are a newer
    /// version's.
    pub(super) fn raw_kind(&self, node: TreeID) -> Option<BlockKind> {
        let kind = BlockKind::read(&self.tree.get_meta(node).ok()?).ok()?;
        (!matches!(kind, BlockKind::Unknown { .. })).then_some(kind)
    }

    /// Whether a row or cell sits where it belongs: a row in a table, a cell in a row (invariant 9).
    pub(super) fn in_place(&self, node: TreeID, kind: &BlockKind) -> bool {
        let parent = match self.tree.parent(node) {
            Some(TreeParentId::Node(parent)) => self.raw_kind(parent),
            _ => None,
        };
        parent.and_then(|parent| parent.fixed_child()).as_ref() == Some(kind)
    }
}

#[cfg(test)]
mod tests;
