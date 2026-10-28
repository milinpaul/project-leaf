//! Editing a block's text: inserting, deleting, and applying or removing marks.
//!
//! Positions are Unicode scalar values (what Loro calls "unicode" positions), not bytes and not
//! grapheme clusters. Checking every range against the text length first means a bad position is a
//! clear error rather than a partial edit.

use std::ops::Range;

use leaf_core::BlockId;
use loro::LoroText;

use super::PageDoc;
use crate::{Mark, MarkKind, ModelError, PageOutcome};

impl PageDoc {
    pub(super) fn insert_text(
        &mut self,
        id: BlockId,
        pos: usize,
        text: &str,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        check_range(&container, pos..pos)?;
        if !text.is_empty() {
            container.insert(pos, text)?;
        }
        Ok(PageOutcome::Changed)
    }

    pub(super) fn delete_text(
        &mut self,
        id: BlockId,
        range: Range<usize>,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        check_range(&container, range.clone())?;
        if !range.is_empty() {
            container.delete(range.start, range.len())?;
        }
        Ok(PageOutcome::Changed)
    }

    pub(super) fn set_mark(
        &mut self,
        id: BlockId,
        range: Range<usize>,
        mark: &Mark,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        let kind = self.kind(id)?;
        if !kind.allows_marks() {
            return Err(ModelError::MarksNotAllowed {
                kind: kind.type_name().to_owned(),
            });
        }
        if matches!(mark, Mark::Link(href) if href.is_empty()) {
            return Err(ModelError::EmptyLink);
        }
        check_range(&container, range.clone())?;
        if !range.is_empty() {
            container.mark(range, mark.kind().key(), mark.value())?;
        }
        Ok(PageOutcome::Changed)
    }

    pub(super) fn remove_mark(
        &mut self,
        id: BlockId,
        range: Range<usize>,
        kind: MarkKind,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        check_range(&container, range.clone())?;
        if !range.is_empty() {
            container.unmark(range, kind.key())?;
        }
        Ok(PageOutcome::Changed)
    }

    /// The block's text container, or an error for blocks without text. Unknown kinds count as
    /// having none even if a newer version gave them text: they are never rewritten.
    pub(super) fn editable_text(&self, id: BlockId) -> Result<LoroText, ModelError> {
        let kind = self.kind(id)?;
        match self.text_container(id)? {
            Some(text) if kind.has_text() => Ok(text),
            _ => Err(ModelError::TextNotAllowed {
                kind: kind.type_name().to_owned(),
            }),
        }
    }
}

/// Fails unless `range` is well-formed and within the text.
fn check_range(text: &LoroText, range: Range<usize>) -> Result<(), ModelError> {
    let len = text.len_unicode();
    if range.start > range.end || range.end > len {
        return Err(ModelError::TextOutOfRange {
            end: range.end.max(range.start),
            len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
