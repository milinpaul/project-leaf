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
}

impl PageEditor {
    /// The blocks an action from `block`'s menu applies to: the selection when `block` is part of it,
    /// otherwise `block` alone. In reading order.
    pub fn action_targets(&mut self, block: BlockId) -> Result<Vec<BlockId>, EditorError> {
        let targets = match self.selection()? {
            Some(Selection::Blocks(blocks)) if self.covered_by(block, &blocks)? => blocks,
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
        Ok(targets.into_iter().filter(|&b| self.doc.contains(b)).collect())
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
                    let unknown = matches!(self.doc.kind(target)?, BlockKind::Unknown { .. });
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
            BlockAction::Delete => {}
        }
        self.finish()?;
        self.caret()
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
mod tests;
