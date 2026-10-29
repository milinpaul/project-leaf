//! Up and Down: moving the caret between visual lines, inside a block and across blocks.
//!
//! Where a block's text wraps depends on fonts and window width, which only the UI knows. The UI
//! describes its layout through [`LineLayout`]; the rules for where the caret goes stay here, plain
//! and testable (tests use a fake monospace layout). The caret keeps a preferred x while Up and Down
//! repeat, so passing a short line doesn't drag it to the left for good (ADR 0016).

use leaf_core::BlockId;
use leaf_model::Caret;

use super::Direction;
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
        let block = caret.block;
        let x = self
            .caret
            .as_ref()
            .and_then(|tracked| tracked.goal_x)
            .unwrap_or_else(|| layout.offset_to_x(block, caret.at));
        let line = layout.line_of(block, caret.at);
        let last_line = layout.line_count(block).max(1) - 1;
        let target = match direction {
            Vertical::Up if line > 0 => Caret {
                block,
                at: layout.x_to_offset(block, line - 1, x),
            },
            Vertical::Down if line < last_line => Caret {
                block,
                at: layout.x_to_offset(block, line + 1, x),
            },
            Vertical::Up => match self.neighbour(block, Direction::Back)? {
                Some((above, _)) => Caret {
                    block: above,
                    at: layout.x_to_offset(above, layout.line_count(above).max(1) - 1, x),
                },
                None => Caret { block, at: 0 },
            },
            Vertical::Down => match self.neighbour(block, Direction::Forward)? {
                Some((below, _)) => Caret {
                    block: below,
                    at: layout.x_to_offset(below, 0, x),
                },
                None => Caret {
                    block,
                    at: self.text_len(block)?,
                },
            },
        };
        self.place(target, Some(x)).map(Some)
    }

    fn text_len(&self, block: BlockId) -> Result<usize, EditorError> {
        Ok(self.doc.plain_text(block)?.map_or(0, |text| text.chars().count()))
    }
}

#[cfg(test)]
mod tests;
