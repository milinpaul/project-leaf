//! Editing a block's text: inserting, deleting, replacing, and applying or removing marks.
//!
//! Positions are Unicode scalar values (what Loro calls "unicode" positions), not bytes and not
//! grapheme clusters. Checking every range against the text length first means a bad position is a
//! clear error rather than a partial edit.

use std::collections::HashMap;
use std::ops::Range;

use leaf_core::BlockId;
use loro::{LoroText, LoroValue, TextDelta, cursor::PosType};

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

    pub(super) fn replace_text(
        &mut self,
        id: BlockId,
        range: Range<usize>,
        text: &str,
    ) -> Result<PageOutcome, ModelError> {
        let container = self.editable_text(id)?;
        check_range(&container, range.clone())?;
        if range.is_empty() {
            return self.insert_text(id, range.start, text);
        }
        let wanted = attributes_at(&container, range.start)?;
        container.delete(range.start, range.len())?;
        if text.is_empty() {
            return Ok(PageOutcome::Changed);
        }
        container.insert(range.start, text)?;
        let added = range.start..range.start + text.chars().count();
        // The expand rules gave the new text the marks of its neighbours; make them the replaced
        // text's instead. Unknown marks from newer versions are carried over too.
        let got = attributes_at(&container, range.start)?;
        for (key, value) in &wanted {
            if got.get(key) != Some(value) {
                container.mark(added.clone(), key, value.clone())?;
            }
        }
        for key in got.keys().filter(|key| !wanted.contains_key(*key)) {
            container.unmark(added.clone(), key)?;
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
        let value = mark.value();
        let key = mark.kind().key();
        if range.is_empty()
            || mark_values(&container, &range, key)?
                .iter()
                .all(|v| v.as_ref() == Some(&value))
        {
            return Ok(PageOutcome::Unchanged);
        }
        container.mark(range, key, value)?;
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
        if range.is_empty()
            || mark_values(&container, &range, kind.key())?
                .iter()
                .all(Option::is_none)
        {
            return Ok(PageOutcome::Unchanged);
        }
        container.unmark(range, kind.key())?;
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

/// The value of mark `key` on each run of text in `range` (`None` where it is absent).
///
/// Mark commands that would change nothing must write nothing: Loro records an op for them but
/// no undo step, so the editor would wait for an undo that never comes (E02-S11).
fn mark_values(
    text: &LoroText,
    range: &Range<usize>,
    key: &str,
) -> Result<Vec<Option<LoroValue>>, ModelError> {
    Ok(text
        .slice_delta(range.start, range.end, PosType::Unicode)?
        .into_iter()
        .filter_map(|delta| match delta {
            TextDelta::Insert { attributes, .. } => Some(
                attributes
                    .and_then(|mut attrs| attrs.remove(key))
                    .filter(|v| !matches!(v, LoroValue::Null)),
            ),
            _ => None,
        })
        .collect())
}

/// The marks on the character at `pos`, as stored (absent and null both mean "not set").
fn attributes_at(text: &LoroText, pos: usize) -> Result<HashMap<String, LoroValue>, ModelError> {
    Ok(text
        .slice_delta(pos, pos + 1, PosType::Unicode)?
        .into_iter()
        .find_map(|delta| match delta {
            TextDelta::Insert { attributes, .. } => attributes,
            _ => None,
        })
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, value)| !matches!(value, LoroValue::Null))
        .collect())
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
mod replace_tests;
#[cfg(test)]
mod tests;
