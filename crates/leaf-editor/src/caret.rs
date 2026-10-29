//! The page editor's caret: where it is, how arrow keys and Home/End move it, and how it follows
//! changes made by anything else (undo, redo, another device's edits).
//!
//! The caret is kept as a model [`Anchor`], not a plain number, so text typed or deleted before it
//! moves it along. It always sits on a grapheme boundary (see [`motion`]). If its block disappears,
//! it moves to the nearest block above where the block was.

pub(crate) mod motion;
mod vertical;

pub use vertical::{LineLayout, Vertical};

use leaf_core::BlockId;
use leaf_model::{Anchor, Caret};

use crate::page_editor::PageEditor;
use crate::{BlockRow, EditorError};

/// A caret movement, from the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Left arrow: one character back, or to the end of the previous block.
    Left,
    /// Right arrow: one character on, or to the start of the next block.
    Right,
    /// Option+Left (Ctrl+Left elsewhere): to the start of this or the previous word.
    WordLeft,
    /// Option+Right (Ctrl+Right elsewhere): to the end of this or the next word.
    WordRight,
    /// Home (Cmd+Left on macOS): the start of the line.
    LineStart,
    /// End (Cmd+Right on macOS): the end of the line.
    LineEnd,
}

/// The caret as the editor keeps it.
pub(crate) struct Tracked {
    anchor: Anchor,
    /// Its block's place in reading order when last seen, to recover if the block is deleted.
    row: usize,
    /// The x position Up and Down aim for, kept while they repeat so short lines don't pull the
    /// caret sideways for good.
    goal_x: Option<f32>,
    /// The other end of a text selection in the same block, if there is one; the caret is the end
    /// that moves. Any caret move drops it.
    pub(crate) tail: Option<Anchor>,
}

impl PageEditor {
    /// Puts the caret at `caret` (snapped to the start of the character it falls in).
    pub fn set_caret(&mut self, caret: Caret) -> Result<Caret, EditorError> {
        self.place(caret, None)
    }

    /// Puts the caret at `caret`, remembering `goal_x` for the next Up or Down.
    fn place(&mut self, caret: Caret, goal_x: Option<f32>) -> Result<Caret, EditorError> {
        let text = self.doc.plain_text(caret.block)?.unwrap_or_default();
        let caret = Caret {
            at: motion::snap(&text, caret.at),
            ..caret
        };
        let anchor = self.doc.anchor(caret)?;
        let row = self
            .view()?
            .iter()
            .position(|row| row.id == caret.block)
            .unwrap_or(0);
        self.caret = Some(Tracked {
            anchor,
            row,
            goal_x,
            tail: None,
        });
        Ok(caret)
    }

    /// Where the caret is now, after whatever changed since it was set. `None` before it is set, or
    /// once the page has no blocks left.
    pub fn caret(&mut self) -> Result<Option<Caret>, EditorError> {
        let Some(tracked) = &self.caret else {
            return Ok(None);
        };
        let (row, goal_x, tail) = (tracked.row, tracked.goal_x, tracked.tail.clone());
        let found = match self.doc.resolve(&tracked.anchor)? {
            Some(caret) => Some(caret),
            None => nearest_above(self.view()?, row),
        };
        // Re-anchoring keeps following edits even after undo re-created the block.
        if let Some(caret) = found {
            let caret = self.place(caret, goal_x)?;
            if let Some(tracked) = &mut self.caret {
                tracked.tail = tail;
            }
            return Ok(Some(caret));
        }
        self.caret = None;
        Ok(None)
    }

    /// Moves the caret and returns where it went. Does nothing before the caret is set.
    pub fn move_caret(&mut self, motion: Motion) -> Result<Option<Caret>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        let text = self.doc.plain_text(caret.block)?.unwrap_or_default();
        let len = text.chars().count();
        let within = |at| Some(Caret { at, ..caret });
        let target = match motion {
            Motion::Left | Motion::WordLeft if caret.at == 0 => self.across(caret, Direction::Back)?,
            Motion::Right | Motion::WordRight if caret.at >= len => self.across(caret, Direction::Forward)?,
            Motion::Left => within(motion::previous_grapheme(&text, caret.at)),
            Motion::Right => within(motion::next_grapheme(&text, caret.at)),
            Motion::WordLeft => within(motion::word_left(&text, caret.at)),
            Motion::WordRight => within(motion::word_right(&text, caret.at)),
            Motion::LineStart => within(motion::line_start(&text, caret.at)),
            Motion::LineEnd => within(motion::line_end(&text, caret.at)),
        };
        self.set_caret(target.unwrap_or(caret)).map(Some)
    }

    /// The end of the previous text block, or the start of the next one.
    fn across(&mut self, caret: Caret, direction: Direction) -> Result<Option<Caret>, EditorError> {
        Ok(self.neighbour(caret.block, direction)?.map(|(block, len)| Caret {
            block,
            at: match direction {
                Direction::Back => len,
                Direction::Forward => 0,
            },
        }))
    }

    /// The text block before or after `block` in reading order, with its text length. Blocks
    /// without text (dividers, images) are passed over.
    fn neighbour(
        &mut self,
        block: BlockId,
        direction: Direction,
    ) -> Result<Option<(BlockId, usize)>, EditorError> {
        let rows = self.view()?;
        let Some(here) = rows.iter().position(|row| row.id == block) else {
            return Ok(None);
        };
        let text_row = |row: &BlockRow| row.text.as_ref().map(|text| (row.id, text.chars().count()));
        Ok(match direction {
            Direction::Back => rows[..here].iter().rev().find_map(text_row),
            Direction::Forward => rows[here + 1..].iter().find_map(text_row),
        })
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Back,
    Forward,
}

fn end_of(row: &BlockRow) -> Option<Caret> {
    row.text.as_ref().map(|text| Caret {
        block: row.id,
        at: text.chars().count(),
    })
}

fn start_of(row: &BlockRow) -> Option<Caret> {
    row.text.as_ref().map(|_| Caret { block: row.id, at: 0 })
}

/// Where a caret goes when its block (once at `row`) was deleted: the end of the nearest text block
/// above, else the start of the first text block below, else any block.
pub(crate) fn nearest_above(rows: &[BlockRow], row: usize) -> Option<Caret> {
    let split = row.min(rows.len());
    rows[..split]
        .iter()
        .rev()
        .find_map(end_of)
        .or_else(|| rows[split..].iter().find_map(start_of))
        .or_else(|| {
            rows.first().map(|first| Caret {
                block: first.id,
                at: 0,
            })
        })
}

#[cfg(test)]
mod tests;
