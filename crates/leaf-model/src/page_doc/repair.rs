//! Putting the page's rules back after merging another copy's changes (E02-S09, ADR-044).
//!
//! Each copy only ever applies commands that keep the invariants, but two copies changing the same
//! block at once can merge into a page that breaks them: one turns a block into code while the other
//! nests a block under it, or bolds a word in it. CRDT merges always agree, so every copy sees the
//! same broken page; each one then makes the same repairs, which merge cleanly too. Nothing the user
//! wrote is dropped: children step out of a block that can't hold them (as when converting a block
//! locally), marks leave code text, a text block that lost its text gets an empty one, and a
//! heading that lost its level gets level 1.

use loro::{Container, TreeID, ValueOrContainer};

use super::PageDoc;
use super::kind_ops::strip_marks;
use super::page_links::repair_links;
use crate::block_kind::{KNOWN_PROP_KEYS, keys};
use crate::{BlockKind, ModelError};

/// The commit origin of repairs, which the undo manager leaves out: they aren't the user's to undo.
pub(super) const REPAIR_ORIGIN: &str = "leaf-repair";

impl PageDoc {
    /// Repairs every block that breaks invariant 2, 3, 4, 5 or 10, as one commit outside the undo history.
    /// Blocks of unknown kinds are left alone (their rules are a newer version's). Returns whether
    /// anything changed.
    pub(super) fn repair(&mut self) -> Result<bool, ModelError> {
        let mut changed = false;
        // Each fix is local, so the result doesn't depend on the order blocks are visited in. A lifted
        // child may itself need repair in its new place, hence passes until one changes nothing.
        loop {
            let mut pass = false;
            for node in self.tree.get_nodes(false).into_iter().map(|n| n.id) {
                pass |= self.repair_block(node)?;
            }
            if !pass {
                break;
            }
            changed = true;
        }
        if changed {
            self.doc.set_next_commit_origin(REPAIR_ORIGIN);
            self.doc.commit();
            self.index = super::BlockIndex::build(&self.tree)?;
        }
        Ok(changed)
    }

    fn repair_block(&mut self, node: TreeID) -> Result<bool, ModelError> {
        let meta = self.tree.get_meta(node)?;
        let mut changed = false;
        let kind = match BlockKind::read(&meta) {
            Ok(kind) => kind,
            // 4: a property the kind needs went missing, because the other copy changed the block to
            // a kind without it at the same time. A heading falls back to level 1; an image without
            // its file becomes a paragraph, keeping its caption.
            Err((field, _)) => {
                let fallback = match field {
                    keys::LEVEL => BlockKind::Heading { level: 1 },
                    keys::HEADER_ROW => BlockKind::Table { header_row: false },
                    keys::ASSET => BlockKind::Paragraph,
                    _ => return Ok(false),
                };
                fallback.write(&meta)?;
                for key in KNOWN_PROP_KEYS
                    .iter()
                    .filter(|key| !fallback.prop_keys().contains(key))
                {
                    if meta.get(key).is_some() {
                        meta.delete(key)?;
                    }
                }
                changed = true;
                fallback
            }
        };
        if matches!(kind, BlockKind::Unknown { .. }) {
            return Ok(changed);
        }
        let text = match meta.get(keys::TEXT) {
            Some(ValueOrContainer::Container(Container::Text(text))) => Some(text),
            _ => None,
        };
        match (&text, kind.has_text()) {
            // 2: a text block without text gets an empty one.
            (None, true) => {
                meta.ensure_mergeable_text(keys::TEXT)?;
                changed = true;
            }
            // 2: a divider holding words becomes a paragraph, so the words stay; an empty one loses
            // the container.
            (Some(text), false) if text.len_unicode() > 0 => {
                BlockKind::Paragraph.write(&meta)?;
                changed = true;
            }
            (Some(_), false) => {
                meta.delete(keys::TEXT)?;
                changed = true;
            }
            _ => {}
        }
        // 5: code text carries no marks, so no links either; 10: links stay whole elsewhere.
        if let (Some(text), true) = (&text, kind.has_text()) {
            changed |= repair_links(text, kind.allows_marks())?;
        }
        if let (Some(text), true) = (&text, kind.has_text() && !kind.allows_marks()) {
            changed |= strip_marks(text)?;
        }
        // 3: children step out of a block that can't hold them; a table or row keeps only its rows
        // or cells.
        let kind = BlockKind::read(&meta).unwrap_or(kind);
        match kind.fixed_child() {
            Some(allowed) => changed |= self.lift_children_except(node, &allowed)?,
            None if !kind.allows_children() => changed |= self.lift_children(node)?,
            None => {}
        }
        // 9: a row or cell outside its table becomes a paragraph, keeping its text and what it holds
        // (the next pass gives it a text container and lets its own rows or cells out).
        if kind.is_table_part() && !self.in_place(node, &kind) {
            BlockKind::Paragraph.write(&meta)?;
            changed = true;
        }
        Ok(changed)
    }

    /// Moves every child of `node` that isn't of kind `keep` out to sit right after it, in order.
    fn lift_children_except(&self, node: TreeID, keep: &BlockKind) -> Result<bool, ModelError> {
        let mut previous = node;
        let mut moved = false;
        for child in self.tree.children(node).unwrap_or_default() {
            if self.raw_kind(child).is_some_and(|kind| kind != *keep) {
                self.tree.mov_after(child, previous)?;
                previous = child;
                moved = true;
            }
        }
        Ok(moved)
    }
}

#[cfg(test)]
mod tests;
