//! Turning a block into another kind, and changing its properties.
//!
//! Everything that could fail is checked before the first write, so a refused conversion leaves the
//! block exactly as it was.

use leaf_core::BlockId;
use loro::TextDelta;

use super::PageDoc;
use crate::block_kind::{KNOWN_PROP_KEYS, keys};
use crate::{BlockKind, ModelError, PageOutcome};

impl PageDoc {
    pub(super) fn set_kind(&mut self, id: BlockId, kind: &BlockKind) -> Result<PageOutcome, ModelError> {
        kind.check_creatable()?;
        let node = self.node(id)?;
        let old = self.kind(id)?;
        if let BlockKind::Unknown { type_name } = &old {
            return Err(ModelError::InvalidKind(format!(
                "cannot change a block of unknown type {type_name:?}: it would be rewritten"
            )));
        }
        if old == *kind {
            return Ok(PageOutcome::Changed); // nothing to write; apply reports Unchanged
        }
        let text = self.text_container(id)?;
        if !kind.has_text() && text.as_ref().is_some_and(|t| t.len_unicode() > 0) {
            return Err(ModelError::TextNotAllowed {
                kind: kind.type_name().to_owned(),
            });
        }

        let meta = self.tree.get_meta(node)?;
        kind.write(&meta)?;
        for key in KNOWN_PROP_KEYS
            .iter()
            .filter(|key| !kind.prop_keys().contains(key))
        {
            if meta.get(key).is_some() {
                meta.delete(key)?;
            }
        }
        match (&text, kind.has_text()) {
            (None, true) => {
                meta.ensure_mergeable_text(keys::TEXT)?;
            }
            (Some(_), false) => meta.delete(keys::TEXT)?,
            _ => {}
        }
        if let Some(text) = text.filter(|_| kind.has_text() && !kind.allows_marks()) {
            // Code text carries no marks (invariant 5): strip every one, including marks from newer versions.
            let mut mark_keys: Vec<String> = text
                .to_delta()
                .into_iter()
                .filter_map(|delta| match delta {
                    TextDelta::Insert { attributes, .. } => attributes,
                    _ => None,
                })
                .flatten()
                .map(|(key, _)| key)
                .collect();
            mark_keys.sort_unstable();
            mark_keys.dedup();
            let len = text.len_unicode();
            for key in mark_keys {
                text.unmark(0..len, &key)?;
            }
        }
        if !kind.allows_children() {
            // Children step out to sit right after the block, keeping their order.
            let mut previous = node;
            for child in self.tree.children(node).unwrap_or_default() {
                self.tree.mov_after(child, previous)?;
                previous = child;
            }
        }
        Ok(PageOutcome::Changed)
    }
}

#[cfg(test)]
mod tests;
