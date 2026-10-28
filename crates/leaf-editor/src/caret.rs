//! The page editor's caret: where it is, how arrow keys and Home/End move it, and how it follows
//! changes made by anything else (undo, redo, another device's edits).
//!
//! The caret is kept as a model [`Anchor`], not a plain number, so text typed or deleted before it
//! moves it along. It always sits on a grapheme boundary (see [`motion`]). If its block disappears,
//! it moves to the nearest block above where the block was.

pub(crate) mod motion;

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
}

impl PageEditor {
    /// Puts the caret at `caret` (snapped to the start of the character it falls in).
    pub fn set_caret(&mut self, caret: Caret) -> Result<Caret, EditorError> {
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
        self.caret = Some(Tracked { anchor, row });
        Ok(caret)
    }

    /// Where the caret is now, after whatever changed since it was set. `None` before it is set, or
    /// once the page has no blocks left.
    pub fn caret(&mut self) -> Result<Option<Caret>, EditorError> {
        let Some(tracked) = &self.caret else {
            return Ok(None);
        };
        let row = tracked.row;
        let found = match self.doc.resolve(&tracked.anchor)? {
            Some(caret) => Some(caret),
            None => nearest_above(self.view()?, row),
        };
        // Re-anchoring keeps following edits even after undo re-created the block.
        if let Some(caret) = found {
            return self.set_caret(caret).map(Some);
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

    /// The end of the previous text block, or the start of the next one. Blocks without text
    /// (dividers, images) are passed over.
    fn across(&mut self, caret: Caret, direction: Direction) -> Result<Option<Caret>, EditorError> {
        let rows = self.view()?;
        let Some(here) = rows.iter().position(|row| row.id == caret.block) else {
            return Ok(None);
        };
        Ok(match direction {
            Direction::Back => rows[..here].iter().rev().find_map(end_of),
            Direction::Forward => rows[here + 1..].iter().find_map(start_of),
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
fn nearest_above(rows: &[BlockRow], row: usize) -> Option<Caret> {
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
