//! Typed text and input-method composition.
//!
//! Typing replaces the selection (or inserts at the caret) with one `ReplaceText` command, so it is one
//! undo step. Input methods (Japanese, Chinese, Korean, Indic keyboards, dead keys) first show
//! *preedit* text: a draft, underlined, that changes with each key until the user commits it. The
//! draft lives only here in the editor and is drawn over the document, never written to it, so a
//! composition makes no undo steps, no saves and nothing for other devices until it is committed.
//! An empty draft is the same as none: it never deletes the selection (ADR 0017).

use std::ops::Range;

use leaf_core::BlockId;
use leaf_model::{Caret, PageCommand};

use crate::caret::motion;
use crate::{EditorError, PageEditor};

/// A text selection inside one block. Selections across blocks come with E04-S06.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange {
    /// The block.
    pub block: BlockId,
    /// Unicode scalar positions, `start <= end`.
    pub range: Range<usize>,
}

/// The input method's draft, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preedit {
    /// Where it is drawn: the block, and the text it covers (the selection it will replace, or an
    /// empty range at the caret). The covered text is hidden while the draft shows.
    pub at: TextRange,
    /// The draft text.
    pub text: String,
    /// Where the input method's own cursor is inside the draft, in Unicode scalars.
    pub cursor: usize,
}

/// The draft as the editor keeps it; its position is taken from the caret when asked.
pub(crate) struct Composing {
    text: String,
    cursor: usize,
}

impl PageEditor {
    /// Selects `range` in `block`, with the caret at its end. Both ends snap to character starts.
    pub fn select(&mut self, block: BlockId, range: Range<usize>) -> Result<(), EditorError> {
        let start = self.set_caret(Caret {
            block,
            at: range.start,
        })?;
        let start = self.doc.anchor(start)?;
        self.set_caret(Caret { block, at: range.end })?;
        if let Some(tracked) = &mut self.caret {
            tracked.tail = Some(start);
        }
        Ok(())
    }

    /// The selected text, after whatever changed since it was made. `None` if nothing is selected
    /// or the selection collapsed to the caret.
    pub fn selection(&mut self) -> Result<Option<TextRange>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        let Some(tail) = self.caret.as_ref().and_then(|tracked| tracked.tail.clone()) else {
            return Ok(None);
        };
        let Some(other) = self.doc.resolve(&tail)?.filter(|t| t.block == caret.block) else {
            return Ok(None);
        };
        let range = caret.at.min(other.at)..caret.at.max(other.at);
        Ok((!range.is_empty()).then_some(TextRange {
            block: caret.block,
            range,
        }))
    }

    /// Types `text`: replaces the selection, or inserts at the caret, as one undo step, and puts
    /// the caret after it. Empty text, no caret, or a block without text (a divider) change nothing.
    pub fn insert_text(&mut self, text: &str) -> Result<Option<Caret>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        if text.is_empty() || self.doc.plain_text(caret.block)?.is_none() {
            return Ok(Some(caret));
        }
        let target = self.selection()?.unwrap_or(TextRange {
            block: caret.block,
            range: caret.at..caret.at,
        });
        // Undo puts the caret back where it was before the typing.
        self.doc.set_caret(Some(caret));
        self.doc.apply(PageCommand::ReplaceText {
            id: target.block,
            range: target.range.clone(),
            text: text.to_owned(),
        })?;
        let after = Caret {
            block: target.block,
            at: target.range.start + text.chars().count(),
        };
        self.set_caret(after).map(Some)
    }

    /// The input method's draft changed. `cursor` is its own cursor inside `text`, in Unicode
    /// scalars. Nothing is written to the page; an empty draft ends the composition and leaves the
    /// selection alone.
    pub fn set_preedit(&mut self, text: &str, cursor: usize) {
        self.composing = (!text.is_empty()).then(|| Composing {
            text: text.to_owned(),
            cursor: motion::snap(text, cursor),
        });
    }

    /// The input method committed `text`: it replaces the selection like typing, as one undo step.
    /// Committing nothing just ends the composition.
    pub fn commit_preedit(&mut self, text: &str) -> Result<Option<Caret>, EditorError> {
        self.composing = None;
        self.insert_text(text)
    }

    /// The composition was abandoned (Escape, focus lost): the draft disappears, the page and the
    /// selection are as before.
    pub fn cancel_preedit(&mut self) {
        self.composing = None;
    }

    /// The draft to draw, if a composition is going on and the caret is in a text block.
    pub fn preedit(&mut self) -> Result<Option<Preedit>, EditorError> {
        let Some((text, cursor)) = self
            .composing
            .as_ref()
            .map(|draft| (draft.text.clone(), draft.cursor))
        else {
            return Ok(None);
        };
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        if self.doc.plain_text(caret.block)?.is_none() {
            return Ok(None);
        }
        let at = self.selection()?.unwrap_or(TextRange {
            block: caret.block,
            range: caret.at..caret.at,
        });
        Ok(Some(Preedit { at, text, cursor }))
    }
}

#[cfg(test)]
mod tests;
