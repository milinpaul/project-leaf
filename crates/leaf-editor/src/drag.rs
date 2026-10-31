//! Dragging blocks by their ⋮⋮ handle: where a drop lands for a pointer position, and the move
//! itself (E05-S04, ADR-049).
//!
//! The UI reports where it drew the rows ([`RowBox`]) and where the pointer is; everything else is
//! worked out here from the page, so it is tested without a window. A drop always lands in a gap
//! between two rows, the way an outliner works: the pointer's height picks the gap (the nearer edge of
//! the row under it) and its distance from the left edge picks the depth, within what the gap allows.
//! The blocks being dragged are left out first, so a drop can never land inside them.

use leaf_core::BlockId;
use leaf_model::PageCommand;

use crate::{EditorError, PageEditor};

/// Where the UI drew one block's row, in the same coordinates as the pointer. Only the rows on screen
/// are needed, in reading order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowBox {
    /// The block whose row it is.
    pub block: BlockId,
    /// The row's top edge.
    pub top: f32,
    /// The row's bottom edge.
    pub bottom: f32,
}

/// Where dragged blocks would land: what the UI draws as a line, and what the move does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDrop {
    /// The row the line is drawn under, or `None` for above the first row. Never a dragged block or
    /// one inside it.
    pub below: Option<BlockId>,
    /// How far the line is indented: the depth the blocks will have.
    pub depth: usize,
    /// The block they go into, or `None` for the top level. Deeper than the row above means nesting
    /// in it.
    pub parent: Option<BlockId>,
    /// The sibling they go right after, or `None` to come first in `parent`.
    pub after: Option<BlockId>,
}

/// The page outline with the dragged blocks (and everything inside them) left out.
struct Outline {
    /// The remaining blocks with their depths, in reading order.
    rows: Vec<(BlockId, usize)>,
    /// For every block of the full outline, how many remaining rows come before it.
    before: Vec<(BlockId, usize)>,
    /// The outermost dragged blocks, in reading order.
    roots: Vec<BlockId>,
}

impl PageEditor {
    /// Where `dragged` (as given by [`PageEditor::action_targets`] for the grabbed handle) would land
    /// for a pointer at (`x`, `y`). `x` is measured from where top-level text starts, and each nesting
    /// level is `indent` wide. `None` when no rows are on screen or nothing is dragged.
    pub fn drop_target(
        &mut self,
        dragged: &[BlockId],
        rows: &[RowBox],
        x: f32,
        y: f32,
        indent: f32,
    ) -> Result<Option<BlockDrop>, EditorError> {
        let outline = self.outline_without(dragged)?;
        if outline.roots.is_empty() {
            return Ok(None);
        }
        // The row under the pointer, or the nearest one above or below the rows on screen.
        let Some(row) = rows.iter().find(|row| y < row.bottom).or_else(|| rows.last()) else {
            return Ok(None);
        };
        let below_row = y >= f32::midpoint(row.top, row.bottom);
        // The gap, as a count of remaining rows above it.
        let Some(&(_, before)) = outline.before.iter().find(|(b, _)| *b == row.block) else {
            return Ok(None);
        };
        let row_kept = outline.rows.get(before).is_some_and(|(b, _)| *b == row.block);
        let mut gap = before.saturating_add(usize::from(below_row && row_kept));
        // Nothing goes between a table's cells: a drop there lands right after the table, which the
        // row above then stands for (its cells are listed one level deeper than it).
        while let Some(&(cell, _)) = outline.rows.get(gap) {
            if !self.doc.kind(cell)?.is_table_part() {
                break;
            }
            gap = gap.saturating_add(1);
        }
        let above = match gap.checked_sub(1).and_then(|i| outline.rows.get(i)).copied() {
            Some((cell, depth)) => match crate::table::table_of(&self.doc, cell)? {
                Some(table) => Some((table, depth.saturating_sub(1))),
                None => Some((cell, depth)),
            },
            None => None,
        };
        let next_depth = outline.rows.get(gap).map_or(0, |&(_, depth)| depth);
        let (lowest, deepest) = match above {
            None => (0, 0),
            Some((block, depth)) => {
                let nests = self.doc.kind(block)?.allows_children();
                (next_depth, depth.saturating_add(usize::from(nests)))
            }
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "truncation is the point: each full indent step is one level; negatives are 0"
        )]
        let wanted = if indent > 0.0 { (x / indent) as usize } else { 0 };
        let depth = wanted.clamp(lowest, deepest.max(lowest));
        let (parent, after) = match above {
            None => (None, None),
            Some((block, above_depth)) => {
                // Walk up from the row above to the levels the drop sits at.
                let mut chain = vec![block];
                let mut current = block;
                for _ in 0..above_depth {
                    match self.doc.parent(current)? {
                        Some(parent) => {
                            chain.push(parent);
                            current = parent;
                        }
                        None => break,
                    }
                }
                // `chain[k]` is the ancestor `k` levels up, at depth `above_depth - k`.
                let at = |level: usize| above_depth.checked_sub(level).and_then(|k| chain.get(k)).copied();
                let parent = depth.checked_sub(1).and_then(at);
                let after = if depth <= above_depth { at(depth) } else { None };
                (parent, after)
            }
        };
        Ok(Some(BlockDrop {
            below: above.map(|(block, _)| block),
            depth,
            parent,
            after,
        }))
    }

    /// Moves `dragged`, with everything inside them, to `drop`, keeping their order, as one undo step.
    /// Returns whether anything moved: dropping blocks where they already are changes nothing.
    pub fn drop_blocks(&mut self, dragged: &[BlockId], drop: &BlockDrop) -> Result<bool, EditorError> {
        let outline = self.outline_without(dragged)?;
        let kept = |block: BlockId| outline.rows.iter().any(|&(b, _)| b == block);
        // Nothing to move, or a place inside the dragged blocks (a block can't contain itself).
        if outline.roots.is_empty() || drop.parent.is_some_and(|parent| !kept(parent)) {
            return Ok(false);
        }
        let siblings = match drop.parent {
            Some(parent) => self.doc.children(parent)?,
            None => self.doc.root_blocks(),
        };
        let others: Vec<BlockId> = siblings
            .iter()
            .copied()
            .filter(|b| !outline.roots.contains(b))
            .collect();
        let index = match drop.after {
            Some(after) => match others.iter().position(|&b| b == after) {
                Some(at) => at.saturating_add(1),
                None => return Ok(false), // the page changed under the drag
            },
            None => 0,
        };
        // Already there: the roots sit together in `parent`, right after `index` other siblings.
        let first = siblings.iter().position(|b| outline.roots.contains(b));
        let in_place = first.is_some_and(|first| {
            siblings.get(first..first.saturating_add(outline.roots.len())) == Some(outline.roots.as_slice())
                && siblings.get(..first).is_some_and(|before| {
                    before.iter().filter(|b| !outline.roots.contains(b)).count() == index
                })
        });
        if in_place {
            return Ok(false);
        }
        self.begin()?;
        self.doc.apply(PageCommand::MoveBlocks {
            ids: outline.roots,
            new_parent: drop.parent,
            index,
        })?;
        self.finish()?;
        Ok(true)
    }

    /// The outline without `dragged` and what is inside them.
    fn outline_without(&mut self, dragged: &[BlockId]) -> Result<Outline, EditorError> {
        let mut outline = Outline {
            rows: Vec::new(),
            before: Vec::new(),
            roots: Vec::new(),
        };
        // Depth of the dragged block whose inside is being skipped.
        let mut skipping: Option<usize> = None;
        for row in self.view()? {
            outline.before.push((row.id, outline.rows.len()));
            if skipping.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            skipping = None;
            if dragged.contains(&row.id) {
                outline.roots.push(row.id);
                skipping = Some(row.depth);
            } else {
                outline.rows.push((row.id, row.depth));
            }
        }
        Ok(outline)
    }
}

#[cfg(test)]
mod tests;
