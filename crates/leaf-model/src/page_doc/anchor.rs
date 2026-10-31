//! Carets that follow edits: an [`Anchor`] remembers a text position so that, after other
//! changes (typing elsewhere, another device's edits merged in, undo and redo), it can be turned back
//! into the position that now holds the same spot in the text.
//!
//! An anchor holds a Loro cursor, which is tied to a character rather than a number, plus the caret
//! it was made from. Undo can re-create a block under a new Loro container (ADR-006), which the
//! cursor cannot follow; the block keeps its `BlockId`, so the anchor falls back to the old position,
//! clamped to the text that is there now (ADR-015).

use leaf_core::BlockId;
use loro::ContainerTrait;
use loro::cursor::{Cursor, Side};

use super::PageDoc;
use crate::{Caret, ModelError};

/// A caret that survives changes to the page. Make one with [`PageDoc::anchor`] and turn it back
/// into a [`Caret`] with [`PageDoc::resolve`].
#[derive(Debug, Clone)]
pub struct Anchor {
    caret: Caret,
    cursor: Option<Cursor>,
}

impl Anchor {
    /// The block the caret was in when the anchor was made.
    #[must_use]
    pub fn block(&self) -> BlockId {
        self.caret.block
    }
}

impl PageDoc {
    /// Anchors `caret` to the character it sits before, so it moves with the text around it. A
    /// position past the end of the text is clamped to the end.
    pub fn anchor(&self, caret: Caret) -> Result<Anchor, ModelError> {
        let Some(text) = self.text_container(caret.block)? else {
            return Ok(Anchor {
                caret: Caret { at: 0, ..caret },
                cursor: None,
            });
        };
        let caret = Caret {
            at: caret.at.min(text.len_unicode()),
            ..caret
        };
        Ok(Anchor {
            caret,
            cursor: text.get_cursor(caret.at, Side::Middle),
        })
    }

    /// Where `anchor` points now, or `None` if its block no longer exists.
    pub fn resolve(&self, anchor: &Anchor) -> Result<Option<Caret>, ModelError> {
        let block = anchor.caret.block;
        if !self.contains(block) {
            return Ok(None);
        }
        let Some(text) = self.text_container(block)? else {
            return Ok(Some(Caret { block, at: 0 }));
        };
        let len = text.len_unicode();
        let followed = anchor
            .cursor
            .as_ref()
            // A cursor from another container (the block was re-created by undo) is not followed.
            .filter(|cursor| cursor.container == text.id())
            .and_then(|cursor| self.doc.get_cursor_pos(cursor).ok())
            .map(|found| found.current.pos);
        let at = followed.unwrap_or(anchor.caret.at).min(len);
        Ok(Some(Caret { block, at }))
    }

    /// Merges changes made by another copy of this page (exported with
    /// [`PageDoc::unsaved_changes`] or [`PageDoc::to_bytes`]). The changes are reported by
    /// [`PageDoc::take_changes`] like local ones and are not undoable here. Edits made at the same
    /// time on both copies can merge into a page that breaks the invariants; it is then repaired the
    /// same way on every copy (ADR-044). Phase 0 has no sync; tests use this to check that carets
    /// and views follow other peers' edits and that copies converge.
    pub fn import_changes(&mut self, bytes: &[u8]) -> Result<(), ModelError> {
        crate::loro_import::import(&self.doc, bytes)?;
        self.index = super::BlockIndex::build(&self.tree)?;
        self.repair()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
