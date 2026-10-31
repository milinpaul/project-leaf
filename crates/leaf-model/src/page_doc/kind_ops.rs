//! Turning a block into another kind, and changing its properties.
//!
//! Everything that could fail is checked before the first write, so a refused conversion leaves the
//! block exactly as it was.
//!
//! Web bookmarks have no text, but turn into text and back without losing the address: a bookmark
//! becomes its address as linked text, and a block whose whole text is one link to the address can
//! become a bookmark (ADR-080).

use leaf_core::BlockId;
use loro::{LoroText, TextDelta, TreeID};

use super::PageDoc;
use super::page_links::{mark_run, repair_links};
use crate::block_kind::{KNOWN_PROP_KEYS, keys};
use crate::text::spans;
use crate::{BlockKind, MarkKind, ModelError, PageOutcome, WebPreview};

/// The commit origin of a fetched preview, which the undo manager leaves out: it isn't the owner's edit.
pub(super) const PREVIEW_ORIGIN: &str = "leaf-web-preview";

impl PageDoc {
    pub(super) fn set_kind(&mut self, id: BlockId, kind: &BlockKind) -> Result<PageOutcome, ModelError> {
        kind.check_creatable()?;
        let node = self.node(id)?;
        let old = self.kind(id)?;
        match &old {
            BlockKind::Unknown { type_name } => {
                return Err(ModelError::InvalidKind(format!(
                    "cannot change a block of unknown type {type_name:?}: it would be rewritten"
                )));
            }
            BlockKind::Table { .. } => {
                return Err(ModelError::InvalidKind(
                    "a table can't be turned into another kind of block".into(),
                ));
            }
            // Its database lives and dies with the block (E09-S09): turned into text, it would be lost silently.
            BlockKind::Database { .. } => {
                return Err(ModelError::InvalidKind(
                    "an inline database can't be turned into another kind of block".into(),
                ));
            }
            part if part.is_table_part() => return Err(ModelError::TablePart { block: id }),
            _ => {}
        }
        if matches!(kind, BlockKind::Database { .. }) {
            return Err(ModelError::InvalidKind(
                "an inline database is inserted as a new block, not made from another".into(),
            ));
        }
        if old == *kind {
            return Ok(PageOutcome::Changed); // nothing to write; apply reports Unchanged
        }
        let text = self.text_container(id)?;
        let words = text.as_ref().is_some_and(|t| t.len_unicode() > 0);
        if let BlockKind::WebBookmark { url, .. } = kind
            && words
            && !text.as_ref().is_some_and(|t| is_only_link_to(t, url))
        {
            return Err(ModelError::InvalidKind(
                "only a block whose whole text is one link to the address can become a web bookmark".into(),
            ));
        } else if !kind.has_text() && words && !matches!(kind, BlockKind::WebBookmark { .. }) {
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
        let text = match (text, kind.has_text()) {
            (None, true) => Some(meta.ensure_mergeable_text(keys::TEXT)?),
            // A web bookmark keeps its container, emptied: the words were only its link.
            (Some(text), false) if kind.keeps_text_container() => {
                if words {
                    text.delete(0, text.len_unicode())?;
                }
                None
            }
            (Some(_), false) => {
                meta.delete(keys::TEXT)?;
                None
            }
            (text, true) => text,
            // A divider becoming a web bookmark gets its empty container now, before anything is written
            // into it: made and written in one step, redo would lose the words (ADR-007).
            (None, false) if kind.keeps_text_container() => {
                meta.ensure_mergeable_text(keys::TEXT)?;
                None
            }
            (None, false) => None,
        };
        // A web bookmark becomes its address, linked where the new kind takes marks.
        if let (BlockKind::WebBookmark { url, .. }, Some(text)) = (&old, &text) {
            text.insert(0, url)?;
            if kind.allows_marks() {
                let len = text.len_unicode();
                mark_run(
                    text,
                    0..len,
                    MarkKind::Link.key(),
                    loro::LoroValue::String(url.as_str().into()),
                )?;
            }
        }
        if let Some(text) = text.filter(|_| !kind.allows_marks()) {
            // Code holds no links: they go (undo brings them back), never left as bare placeholders.
            repair_links(&text, false)?;
            strip_marks(&text)?;
        }
        if !kind.allows_children() {
            self.lift_children(node)?;
        }
        Ok(PageOutcome::Changed)
    }

    /// [`PageCommand::SetWebBookmarkPreview`](crate::PageCommand::SetWebBookmarkPreview): fills in the
    /// preview if the block is still a web bookmark for `url`, in a commit the undo history leaves out.
    pub(super) fn set_web_bookmark_preview(
        &mut self,
        id: BlockId,
        url: &str,
        preview: &WebPreview,
    ) -> Result<PageOutcome, ModelError> {
        let Ok(node) = self.node(id) else {
            return Ok(PageOutcome::Unchanged);
        };
        match self.kind(id)? {
            BlockKind::WebBookmark {
                url: current,
                preview: old,
            } if current == url => {
                if old == *preview {
                    return Ok(PageOutcome::Unchanged);
                }
            }
            _ => return Ok(PageOutcome::Unchanged),
        }
        crate::web_bookmark::write_preview(&self.tree.get_meta(node)?, preview)?;
        self.doc.set_next_commit_origin(PREVIEW_ORIGIN);
        Ok(PageOutcome::Changed)
    }

    /// Moves `node`'s children out to sit right after it, keeping their order: what happens to the
    /// children of a block that becomes a kind that can't hold them.
    pub(super) fn lift_children(&self, node: TreeID) -> Result<bool, ModelError> {
        let children = self.tree.children(node).unwrap_or_default();
        let mut previous = node;
        for &child in &children {
            self.tree.mov_after(child, previous)?;
            previous = child;
        }
        Ok(!children.is_empty())
    }
}

/// Whether every character of `text` is linked to `url`: a block that is only that link.
fn is_only_link_to(text: &LoroText, url: &str) -> bool {
    spans(text)
        .iter()
        .all(|span| span.marks.page.is_none() && span.marks.link.as_deref() == Some(url))
}

/// Removes every mark from `text`, including marks from newer versions: code text carries none
/// (invariant 5). Returns whether there were any.
pub(super) fn strip_marks(text: &LoroText) -> Result<bool, ModelError> {
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
    for key in &mark_keys {
        text.unmark(0..len, key)?;
    }
    Ok(!mark_keys.is_empty())
}

#[cfg(test)]
mod tests;
