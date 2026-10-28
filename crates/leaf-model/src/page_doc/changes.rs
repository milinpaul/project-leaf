//! Which blocks changed since the editor last looked, so it can redraw only those.
//!
//! Loro reports every change (from commands, undo, redo, and later from other peers) as events on
//! containers. Each event's path names the block node it happened under, so the page records those
//! nodes, plus whether the outline itself changed (a block created, moved or deleted). The editor
//! takes them as `BlockId`s; Loro types never leave the model.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use leaf_core::BlockId;
use loro::event::{Diff, DiffEvent};
use loro::{Index, LoroDoc, Subscription, TreeID};

use super::PageDoc;

/// What changed on a page since the last [`PageDoc::take_changes`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    /// Blocks whose kind, properties or text changed, or that were created or moved. Blocks that
    /// no longer exist are left out.
    pub blocks: HashSet<BlockId>,
    /// Whether blocks were created, moved or deleted, so order, nesting or list numbers may differ.
    pub structure: bool,
}

/// Changes seen by the event callback and not yet taken, by Loro node.
#[derive(Default)]
pub(super) struct Pending {
    nodes: HashSet<TreeID>,
    structure: bool,
    /// Set if the lock was ever poisoned: then nothing is trusted and everything counts as changed.
    lost: bool,
}

/// Starts recording `doc`'s changes into `pending`. Dropping the returned subscription stops it.
pub(super) fn watch(doc: &LoroDoc, pending: &Arc<Mutex<Pending>>) -> Subscription {
    let pending = Arc::clone(pending);
    doc.subscribe_root(Arc::new(move |event: DiffEvent<'_>| {
        let Ok(mut pending) = pending.lock() else {
            return;
        };
        for diff in &event.events {
            for (_, index) in diff.path {
                if let Index::Node(node) = index {
                    pending.nodes.insert(*node);
                }
            }
            if let Diff::Tree(tree) = &diff.diff {
                pending.structure = true;
                pending.nodes.extend(tree.diff.iter().map(|item| item.target));
            }
        }
    }))
}

impl PageDoc {
    /// What changed since the last call (or since the page was created or loaded), and forgets it.
    pub fn take_changes(&self) -> Changes {
        let taken = match self.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(_) => Pending {
                lost: true,
                ..Pending::default()
            },
        };
        if taken.lost {
            return Changes {
                blocks: self.index.by_block.keys().copied().collect(),
                structure: true,
            };
        }
        Changes {
            blocks: taken
                .nodes
                .iter()
                .filter_map(|node| self.index.by_node.get(node).copied())
                .collect(),
            structure: taken.structure,
        }
    }
}
