//! Splitting a block in two (Enter) and merging a block into the one above (Backspace at its start).
//!
//! Both keep formatting: text moves as a Loro delta, marks included, even marks this version doesn't
//! know. Both keep reading order: nothing that came after the cursor ends up before it.

use leaf_core::BlockId;
use loro::cursor::PosType;
use loro::{LoroText, TextDelta, TreeID};

use super::PageDoc;
use super::page_links::mark_run;
use crate::{BlockKind, ModelError, PAGE_LINK, PageOutcome};

impl PageDoc {
    pub(super) fn split_block(&mut self, id: BlockId, pos: usize) -> Result<PageOutcome, ModelError> {
        let text = self.editable_text(id)?;
        if self.kind(id)?.is_table_part() {
            return Err(ModelError::TablePart { block: id });
        }
        let len = text.len_unicode();
        if pos > len {
            return Err(ModelError::TextOutOfRange { end: pos, len });
        }
        let new_kind = match self.kind(id)? {
            kind @ (BlockKind::BulletedList | BlockKind::NumberedList) => kind,
            BlockKind::Todo { .. } => BlockKind::Todo { checked: false },
            // The next action item: not done, no date yet, the same priority (E19-S02).
            BlockKind::Task { priority, .. } => BlockKind::Task {
                done: false,
                due: None,
                time: None,
                priority,
            },
            _ => BlockKind::Paragraph,
        };
        let node = self.node(id)?;
        // With children, the new block becomes the first child so it still reads right after this one.
        let (parent, index) = if self.tree.children_num(node).unwrap_or(0) > 0 {
            (Some(node), 0)
        } else {
            let parent = self.parent_node(node);
            let siblings = self.tree.children(parent).unwrap_or_default();
            (
                parent,
                siblings
                    .iter()
                    .position(|&n| n == node)
                    .map_or(siblings.len(), |p| p + 1),
            )
        };
        let tail = text.slice_delta(pos, len, PosType::Unicode)?;
        let (new, new_text) = self.create_block(parent, index, &new_kind)?;
        if pos < len {
            text.delete(pos, len - pos)?;
            if let Some(new_text) = new_text {
                append_delta(&new_text, tail)?;
            }
        }
        Ok(PageOutcome::BlockCreated(new))
    }

    pub(super) fn merge_into_previous(&mut self, id: BlockId) -> Result<PageOutcome, ModelError> {
        let source = self.editable_text(id)?;
        if self.kind(id)?.is_table_part() {
            return Err(ModelError::TablePart { block: id });
        }
        let node = self.node(id)?;
        let order = self.document_order();
        let position = order.iter().position(|&n| n == node).unwrap_or(0);
        let Some(&previous_node) = position.checked_sub(1).and_then(|p| order.get(p)) else {
            return Ok(PageOutcome::Unchanged); // the first block has nothing above it
        };
        let into = self.block_at(previous_node)?;
        let into_kind = self.kind(into)?;
        // Right after a table, the block above in reading order is its last cell: the table is the
        // block to select, as for a divider or image.
        if into_kind.is_table_part() {
            return Err(ModelError::MergeTargetNotText {
                target: self.table_of(into)?,
            });
        }
        let target = match self.text_container(into)? {
            Some(text) if into_kind.has_text() && !matches!(into_kind, BlockKind::Image { .. }) => text,
            _ => return Err(ModelError::MergeTargetNotText { target: into }),
        };

        let at = target.len_unicode();
        if into_kind.allows_marks() {
            append_delta(&target, source.to_delta())?;
        } else {
            // Code keeps no marks, so no page links either (ADR 0071).
            let words: String = source.to_string().chars().filter(|&c| c != PAGE_LINK).collect();
            target.insert(at, &words)?;
        }
        let into_is_parent = self.parent_node(node) == Some(previous_node);
        for child in self.tree.children(node).unwrap_or_default() {
            if into_is_parent || !into_kind.allows_children() {
                // Into the parent, or a block that can't hold them: they take the merged block's place.
                self.tree.mov_before(child, node)?;
            } else {
                let end = self.tree.children_num(previous_node).unwrap_or(0);
                self.tree.mov_to(child, previous_node, end)?;
            }
        }
        self.tree.delete(node)?;
        self.index.remove(id);
        Ok(PageOutcome::BlockMerged { into, at })
    }

    fn block_at(&self, node: TreeID) -> Result<BlockId, ModelError> {
        self.index
            .by_node
            .get(&node)
            .copied()
            .ok_or_else(|| ModelError::InvalidBlock {
                node: format!("{node:?}"),
                reason: "not indexed".into(),
            })
    }
}

/// Appends formatted text (a delta of inserts) at the end of `text`, keeping each run's marks exactly.
/// Marks absent from a run are cleared on it, so nothing leaks from the text it is appended to.
pub(super) fn append_delta(text: &LoroText, delta: Vec<TextDelta>) -> Result<(), ModelError> {
    for run in delta {
        let TextDelta::Insert { insert, attributes } = run else {
            continue;
        };
        let start = text.len_unicode();
        text.insert(start, &insert)?;
        let end = text.len_unicode();
        // Typing after bold expands bold; appended text must carry only its own marks.
        let inherited: Vec<String> = text
            .slice_delta(start, end, PosType::Unicode)?
            .into_iter()
            .filter_map(|d| match d {
                TextDelta::Insert {
                    attributes: Some(attrs),
                    ..
                } => Some(attrs.into_keys()),
                _ => None,
            })
            .flatten()
            .collect();
        for key in inherited {
            text.unmark(start..end, &key)?;
        }
        for (key, value) in attributes.into_iter().flatten() {
            mark_run(text, start..end, &key, value)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
