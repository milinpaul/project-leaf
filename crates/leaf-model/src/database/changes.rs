//! What changed in a database since the editor last looked, so the table view re-reads only that (E09-S04).
//!
//! Like a page's changes, these come from Loro's events, so commands, undo, redo and merged changes from another
//! device are all seen the same way. Each event names the top-level container it happened in (`cells`, `rows`,
//! `columns`, `schema`, `view`); events in `cells` also name the row, as a map key on the way down.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use leaf_core::PageId;
use loro::event::{Diff, DiffEvent};
use loro::{ContainerID, Index, LoroDoc, Subscription};

use super::{Database, containers};

/// What changed in a database since the last [`Database::take_changes`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent things that may each have changed, read separately by the table view"
)]
pub struct DatabaseChanges {
    /// Rows whose cells changed (rows that no longer exist may be listed too).
    pub rows: HashSet<PageId>,
    /// Whether rows were added, moved, deleted or restored.
    pub order: bool,
    /// Whether columns were added, renamed, retyped, moved, hidden, resized or deleted, or their options changed.
    pub columns: bool,
    /// Whether the sort or the filters changed.
    pub view: bool,
    /// Whether everything must be read again (the change record was lost).
    pub everything: bool,
}

/// Changes seen by the event callback and not yet taken.
#[derive(Default)]
pub(super) struct Pending(DatabaseChanges);

/// Starts recording `doc`'s changes into `pending`. Dropping the returned subscription stops it.
pub(super) fn watch(doc: &LoroDoc, pending: &Arc<Mutex<Pending>>) -> Subscription {
    let pending = Arc::clone(pending);
    doc.subscribe_root(Arc::new(move |event: DiffEvent<'_>| {
        let Ok(mut pending) = pending.lock() else {
            return;
        };
        let changes = &mut pending.0;
        for diff in &event.events {
            let root = diff.path.first().map_or(diff.target, |(id, _)| id);
            let ContainerID::Root { name, .. } = root else {
                changes.everything = true;
                continue;
            };
            match name.as_str() {
                containers::CELLS => {
                    // Each path step is a container and the key it sits under in its parent: the step right after
                    // the `cells` map is a row's map, under the row's ID. (Mergeable maps are root containers of
                    // their own, so the row can't be told from the container's name.)
                    let is_cells =
                        |id: &ContainerID| matches!(id, ContainerID::Root { name, .. } if name.as_str() == containers::CELLS);
                    if let Some(at) = diff.path.iter().position(|(id, _)| is_cells(id))
                        && let Some((_, Index::Key(key))) = at.checked_add(1).and_then(|next| diff.path.get(next))
                        && let Ok(row) = key.parse::<PageId>()
                    {
                        changes.rows.insert(row);
                    }
                    // A row's map created or replaced on the `cells` map itself: its keys are the rows.
                    if let Diff::Map(map) = &diff.diff
                        && is_cells(diff.target)
                    {
                        changes
                            .rows
                            .extend(map.updated.keys().filter_map(|key| key.parse::<PageId>().ok()));
                    }
                }
                containers::ROWS => changes.order = true,
                containers::COLUMNS | containers::SCHEMA => changes.columns = true,
                containers::VIEW => changes.view = true,
                _ => {}
            }
        }
    }))
}

impl Database {
    /// What changed since the last call (or since the database was created or loaded), and forgets it.
    pub fn take_changes(&self) -> DatabaseChanges {
        match self.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut pending.0),
            Err(_) => DatabaseChanges {
                everything: true,
                ..DatabaseChanges::default()
            },
        }
    }
}

#[cfg(test)]
mod tests;
