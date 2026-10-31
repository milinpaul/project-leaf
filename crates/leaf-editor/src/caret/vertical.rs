//! Up and Down: moving the caret between visual lines, inside a block and across blocks.
//!
//! Where a block's text wraps depends on fonts and window width, which only the UI knows. The UI
//! describes its layout through [`LineLayout`]; the rules for where the caret goes stay here, plain
//! and testable (tests use a fake monospace layout). The caret keeps a preferred x while Up and Down
//! repeat, so passing a short line doesn't drag it to the left for good (ADR-016).

use leaf_core::BlockId;
use leaf_model::Caret;

use super::Direction;
use crate::table::cell_place;
use crate::{EditorError, PageEditor};

/// How the screen laid out each block's text. Implemented by `leaf-ui` from its text layout.
///
/// Offsets are Unicode scalar positions in the block's text, as in the model. x is in page
/// coordinates, shared by every block, so a nested block's indentation counts.
pub trait LineLayout {
    /// How many visual lines the block takes: wrapped lines plus newlines, at least 1.
    fn line_count(&self, block: BlockId) -> usize;
    /// The visual line the caret at `offset` is drawn on. At a wrap point, the later line.
    fn line_of(&self, block: BlockId, offset: usize) -> usize;
    /// Where the caret at `offset` is drawn, horizontally.
    fn offset_to_x(&self, block: BlockId, offset: usize) -> f32;
    /// The caret position on `line` closest to `x`.
    fn x_to_offset(&self, block: BlockId, line: usize, x: f32) -> usize;
}

/// Up or Down arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vertical {
    /// Up arrow: the line above, or the last line of the previous block.
    Up,
    /// Down arrow: the line below, or the first line of the next block.
    Down,
}

impl PageEditor {
    /// Moves the caret one visual line up or down (`PRODUCT_SPEC.md` "Editor behaviour": at the
    /// first or last line, into the previous or next block, keeping the horizontal position).
    /// Blocks without text are passed over. Up on the page's first line goes to its start; Down on
    /// the last line goes to its end. Does nothing before the caret is set.
    pub fn move_vertically(
        &mut self,
        direction: Vertical,
        layout: &impl LineLayout,
    ) -> Result<Option<Caret>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        self.blocks = None;
        self.pending = None;
        let block = caret.block;
        let x = self
            .caret
            .as_ref()
            .and_then(|tracked| tracked.goal_x)
            .unwrap_or_else(|| layout.offset_to_x(block, caret.at));
        let line = layout.line_of(block, caret.at);
        let last_line = layout.line_count(block).saturating_sub(1);
        let target = match direction {
            Vertical::Up if line > 0 => Caret {
                block,
                at: layout.x_to_offset(block, line.saturating_sub(1), x),
            },
            Vertical::Down if line < last_line => Caret {
                block,
                at: layout.x_to_offset(block, line.saturating_add(1), x),
            },
            Vertical::Up | Vertical::Down => self.across_lines(caret, direction, x, layout)?,
        };
        self.place(target, Some(x)).map(Some)
    }

    /// Up from a block's first line or Down from its last: the nearest line of the block above or
    /// below. In a table that is the cell above or below in the same column, and past its top or
    /// bottom row the block before or after the table; coming into a table, the cell under `x`.
    fn across_lines(
        &mut self,
        caret: Caret,
        direction: Vertical,
        x: f32,
        layout: &impl LineLayout,
    ) -> Result<Caret, EditorError> {
        let block = caret.block;
        let (back, way) = match direction {
            Vertical::Up => (true, Direction::Back),
            Vertical::Down => (false, Direction::Forward),
        };
        let next = match cell_place(&self.doc, block)? {
            Some(place) => {
                let row = if back {
                    place.row.checked_sub(1)
                } else {
                    place.row.checked_add(1).filter(|&r| r < place.grid.len())
                };
                let same_column = row.and_then(|r| {
                    let cells = place.grid.get(r)?;
                    cells
                        .get(place.column.min(cells.len().saturating_sub(1)))
                        .copied()
                });
                if let Some(cell) = same_column {
                    Some(cell)
                } else {
                    let edge = if back {
                        place.table
                    } else {
                        place.grid.concat().last().copied().unwrap_or(place.table)
                    };
                    self.neighbour(edge, way)?.map(|(b, _)| b)
                }
            }
            None => self.neighbour(block, way)?.map(|(b, _)| b),
        };
        let Some(next) = next else {
            return Ok(Caret {
                block,
                at: if back { 0 } else { self.text_len(block)? },
            });
        };
        let next = self.column_under(next, block, x, layout)?;
        let line = if back {
            layout.line_count(next).saturating_sub(1)
        } else {
            0
        };
        Ok(Caret {
            block: next,
            at: layout.x_to_offset(next, line, x),
        })
    }

    /// Coming into a table from outside it at `target` (a cell in its first or last row): the cell
    /// in that row whose left edge is the last at or before `x`. Any other target stays.
    fn column_under(
        &self,
        target: BlockId,
        from: BlockId,
        x: f32,
        layout: &impl LineLayout,
    ) -> Result<BlockId, EditorError> {
        let Some(place) = cell_place(&self.doc, target)? else {
            return Ok(target);
        };
        if cell_place(&self.doc, from)?.is_some_and(|here| here.table == place.table) {
            return Ok(target);
        }
        let Some(row) = place.grid.get(place.row) else {
            return Ok(target);
        };
        Ok(row
            .iter()
            .copied()
            .filter(|&cell| layout.offset_to_x(cell, 0) <= x)
            .last()
            .or_else(|| row.first().copied())
            .unwrap_or(target))
    }

    /// How many characters of text `block` has (0 for blocks without text).
    pub(crate) fn text_len(&self, block: BlockId) -> Result<usize, EditorError> {
        Ok(self.doc.plain_text(block)?.map_or(0, |text| text.chars().count()))
    }
}

#[cfg(test)]
mod tests;
