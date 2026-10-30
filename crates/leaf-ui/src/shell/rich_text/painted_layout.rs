//! The editor's [`LineLayout`], answered from the text as it was last painted: where lines wrap,
//! and which character sits under an x position. Up and Down use it to move between visual lines
//! (E04-S03), so it must match the pixels exactly; it reads GPUI's own measured layout of each row.
//!
//! Only rows on screen are laid out (the page is a virtual list). For a block that isn't, the answers
//! treat it as one line starting at its first character: moving into it from the screen's edge still
//! lands at a sensible place, and the list then scrolls it into view.

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{Pixels, TextLayout, px};
use leaf_core::BlockId;
use leaf_editor::LineLayout;

use super::pieces::byte_of;
use boundaries::Boundaries;

mod boundaries;

/// One painted row: its text, GPUI's measured layout of it, and its boundaries once first asked for.
#[derive(Clone)]
struct Painted {
    text: String,
    layout: TextLayout,
    boundaries: OnceCell<Rc<Boundaries>>,
}

/// The rows painted in the last frame, by block. Cheap to clone; clones share the same rows.
#[derive(Clone, Default)]
pub(in crate::shell) struct PaintedLayout(Rc<RefCell<HashMap<BlockId, Painted>>>);

impl PaintedLayout {
    /// Records a row's layout. Called while painting, after GPUI has measured and placed the text,
    /// so every recorded layout can answer position questions.
    pub(in crate::shell) fn record(&self, block: BlockId, text: &str, layout: TextLayout) {
        self.0.borrow_mut().insert(
            block,
            Painted {
                text: text.to_owned(),
                layout,
                boundaries: OnceCell::new(),
            },
        );
    }

    /// How many rows were painted as text in the last frame.
    #[cfg(test)]
    pub(in crate::shell) fn len(&self) -> usize {
        self.0.borrow().len()
    }

    /// Whether `block` was painted as text in the last frame.
    pub(in crate::shell) fn contains(&self, block: BlockId) -> bool {
        self.0.borrow().contains_key(&block)
    }

    /// Where GPUI itself draws the boundary at character `offset`, asked one at a time: what the
    /// one-pass boundaries must match.
    #[cfg(test)]
    pub(in crate::shell) fn gpui_position(
        &self,
        block: BlockId,
        offset: usize,
    ) -> Option<gpui::Point<Pixels>> {
        let rows = self.0.borrow();
        let row = rows.get(&block)?;
        row.layout.position_for_index(byte_of(&row.text, offset))
    }

    /// Where the boundary at character `offset` is drawn, from the one-pass boundaries.
    #[cfg(test)]
    pub(in crate::shell) fn drawn_at(&self, block: BlockId, offset: usize) -> Option<gpui::Point<Pixels>> {
        self.position(block, offset)
    }

    /// Starts a new frame: only the rows painted in it will be known.
    pub(in crate::shell) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    /// The character boundary nearest `position` (window coordinates) in `block`'s painted text, if
    /// the row is on screen: on the visual line under `position`, the one closest in x.
    pub(in crate::shell) fn offset_at(&self, block: BlockId, position: gpui::Point<Pixels>) -> Option<usize> {
        let tops = self.line_tops(block)?;
        let line = tops.iter().rposition(|top| *top <= position.y).unwrap_or(0);
        Some(self.nearest_on_line(block, &tops, line, position.x))
    }

    /// Where the caret at `offset` in `block` is drawn (its top) and how tall it is, if painted.
    pub(in crate::shell) fn caret_bounds(
        &self,
        block: BlockId,
        offset: usize,
    ) -> Option<(gpui::Point<Pixels>, Pixels)> {
        let rows = self.0.borrow();
        let row = rows.get(&block)?;
        let at = row.layout.position_for_index(byte_of(&row.text, offset))?;
        Some((at, row.layout.line_height()))
    }

    /// Every character boundary of `block` with where it is drawn, and its visual lines' tops, worked
    /// out once per painted frame (E17-S08).
    fn boundaries(&self, block: BlockId) -> Option<Rc<Boundaries>> {
        let rows = self.0.borrow();
        let row = rows.get(&block)?;
        Some(
            row.boundaries
                .get_or_init(|| Rc::new(Boundaries::of(&row.text, &row.layout)))
                .clone(),
        )
    }

    /// On visual line `line`, the boundary whose x is closest to `x` (the earlier one on a tie).
    fn nearest_on_line(&self, block: BlockId, tops: &[Pixels], line: usize, x: Pixels) -> usize {
        let Some(top) = tops.get(line).or(tops.last()).copied() else {
            return 0;
        };
        self.boundaries(block)
            .and_then(|bounds| {
                bounds
                    .points
                    .iter()
                    .filter(|(_, p)| (p.y - top).abs() < px(0.5))
                    .min_by(|(_, a), (_, b)| {
                        f32::from((a.x - x).abs()).total_cmp(&f32::from((b.x - x).abs()))
                    })
                    .map(|(offset, _)| *offset)
            })
            .unwrap_or(0)
    }

    /// The top of each visual line of `block`, in order, if it was painted.
    fn line_tops(&self, block: BlockId) -> Option<Vec<Pixels>> {
        self.boundaries(block).map(|bounds| bounds.tops.clone())
    }

    fn position(&self, block: BlockId, offset: usize) -> Option<gpui::Point<Pixels>> {
        self.boundaries(block)?.position(offset)
    }
}

impl LineLayout for PaintedLayout {
    fn line_count(&self, block: BlockId) -> usize {
        self.line_tops(block).map_or(1, |tops| tops.len().max(1))
    }

    fn line_of(&self, block: BlockId, offset: usize) -> usize {
        let (Some(tops), Some(at)) = (self.line_tops(block), self.position(block, offset)) else {
            return 0;
        };
        tops.iter().rposition(|top| *top <= at.y + px(0.5)).unwrap_or(0)
    }

    fn offset_to_x(&self, block: BlockId, offset: usize) -> f32 {
        self.position(block, offset).map_or(0.0, |p| f32::from(p.x))
    }

    fn x_to_offset(&self, block: BlockId, line: usize, x: f32) -> usize {
        self.line_tops(block)
            .map_or(0, |tops| self.nearest_on_line(block, &tops, line, px(x)))
    }
}
