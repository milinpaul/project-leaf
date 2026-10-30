//! The block menu's actions: Turn into, Duplicate and Delete.
//!
//! An action opened on a block that is part of the selection applies to the whole selection (every
//! block a text selection touches, or the selected blocks); otherwise to that block alone, the way
//! the ⋮⋮ handle works in other editors. Each action is one undo step, however many blocks it touches
//! (ADR 0028).

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageOutcome};

use crate::{EditorError, PageEditor, Selection};

/// An action from the block menu.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockAction {
    /// Change the blocks' kind, keeping their text. Blocks that can't become it are left alone: a
    /// block with text can't become a divider (its words would be lost), and blocks from a newer
    /// version are never rewritten.
    TurnInto(BlockKind),
    /// Copy the blocks, with everything nested in them, right after each one. The copies end up
    /// selected.
    Duplicate,
    /// Delete the blocks with everything nested in them.
    Delete,
    /// Move the blocks, with everything nested in them, one place up among their siblings. Nothing
    /// happens when they are already first, or don't share a parent ([`PageEditor::can_move`]).
    MoveUp,
    /// Move the blocks one place down among their siblings; the mirror of [`BlockAction::MoveUp`].
    MoveDown,
}

impl PageEditor {
    /// The blocks an action from `block`'s menu applies to: the selection when `block` is part of it,
    /// otherwise `block` alone. In reading order.
    pub fn action_targets(&mut self, block: BlockId) -> Result<Vec<BlockId>, EditorError> {
        let targets = match self.selection()? {
            Some(Selection::Blocks(blocks)) if self.covered_by(block, &blocks)? => blocks,
            Some(Selection::Cells { table, cells }) if block == table || cells.contains(&block) => {
                vec![table]
            }
            Some(Selection::Text { start, end }) => {
                let touched = self.blocks_between(start, end)?;
                if touched.contains(&block) {
                    touched
                } else {
                    vec![block]
                }
            }
            _ => vec![block],
        };
        // A row or cell is never acted on alone: the action takes its whole table (E16-S02).
        let targets: Vec<BlockId> = targets.into_iter().filter(|&b| self.doc.contains(b)).collect();
        crate::table::whole_tables(&self.doc, &targets)
    }

    /// Runs `action` from `block`'s menu on [`PageEditor::action_targets`], as one undo step. Returns
    /// where the caret is afterwards.
    pub fn block_action(
        &mut self,
        block: BlockId,
        action: BlockAction,
    ) -> Result<Option<Caret>, EditorError> {
        let targets = self.action_targets(block)?;
        if targets.is_empty() {
            return self.caret();
        }
        if action == BlockAction::Delete {
            return self.delete_blocks(&targets);
        }
        self.begin()?;
        match action {
            BlockAction::TurnInto(kind) => {
                let mut convertible = Vec::new();
                for &target in &targets {
                    let text = self.doc.plain_text(target)?.unwrap_or_default();
                    // Blocks from newer versions are never rewritten, and a table stays a table.
                    let unknown = matches!(
                        self.doc.kind(target)?,
                        BlockKind::Unknown { .. } | BlockKind::Table { .. }
                    );
                    let loses_text = kind == BlockKind::Divider && !text.is_empty();
                    if !unknown && !loses_text {
                        convertible.push(target);
                    }
                }
                self.doc.as_one_step(|doc| -> Result<(), EditorError> {
                    for id in convertible {
                        doc.apply(PageCommand::SetBlockKind {
                            id,
                            kind: kind.clone(),
                        })?;
                    }
                    Ok(())
                })?;
            }
            BlockAction::Duplicate => {
                if let PageOutcome::BlocksCreated(copies) =
                    self.doc.apply(PageCommand::DuplicateBlocks { ids: targets })?
                {
                    self.select_blocks(&copies)?;
                }
            }
            BlockAction::MoveUp | BlockAction::MoveDown => {
                let up = action == BlockAction::MoveUp;
                if let Some((new_parent, index)) = self.move_place(&targets, up)? {
                    self.doc.apply(PageCommand::MoveBlocks {
                        ids: targets,
                        new_parent,
                        index,
                    })?;
                }
            }
            BlockAction::Delete => {}
        }
        self.finish()?;
        self.caret()
    }

    /// Whether Move up (`up`) or Move down from `block`'s menu would do anything, to grey out the
    /// menu entry.
    pub fn can_move(&mut self, block: BlockId, up: bool) -> Result<bool, EditorError> {
        let targets = self.action_targets(block)?;
        Ok(self.move_place(&targets, up)?.is_some())
    }

    /// Where `MoveBlocks` should put `targets` to move them one place up or down: past the sibling
    /// before the first of them, or after the one after the last. `None` at the edge, or when the
    /// outermost targets have different parents (there is no single "one place" for them then).
    fn move_place(
        &self,
        targets: &[BlockId],
        up: bool,
    ) -> Result<Option<(Option<BlockId>, usize)>, EditorError> {
        let mut roots = Vec::new();
        for &target in targets {
            let nested = match self.doc.parent(target)? {
                Some(parent) => self.covered_by(parent, targets)?,
                None => false,
            };
            if !nested {
                roots.push(target);
            }
        }
        let Some(&first) = roots.first() else {
            return Ok(None);
        };
        let parent = self.doc.parent(first)?;
        for &root in &roots {
            if self.doc.parent(root)? != parent {
                return Ok(None);
            }
        }
        let siblings = match parent {
            Some(parent) => self.doc.children(parent)?,
            None => self.doc.root_blocks(),
        };
        let positions: Vec<usize> = siblings
            .iter()
            .enumerate()
            .filter(|(_, b)| roots.contains(b))
            .map(|(i, _)| i)
            .collect();
        let (Some(&top), Some(&bottom)) = (positions.first(), positions.last()) else {
            return Ok(None);
        };
        // `MoveBlocks` counts the destination index without the moved blocks themselves.
        Ok(if up {
            top.checked_sub(1).map(|index| (parent, index))
        } else if bottom + 1 < siblings.len() {
            Some((parent, bottom + 2 - positions.len()))
        } else {
            None
        })
    }

    /// Whether `block` is one of `blocks` or nested inside one.
    fn covered_by(&self, block: BlockId, blocks: &[BlockId]) -> Result<bool, EditorError> {
        let mut current = Some(block);
        while let Some(b) = current {
            if blocks.contains(&b) {
                return Ok(true);
            }
            current = self.doc.parent(b)?;
        }
        Ok(false)
    }
}

#[cfg(test)]
pub(crate) mod tests;
