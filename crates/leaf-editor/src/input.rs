//! Typed text and input-method composition.
//!
//! Typing replaces the selection (or inserts at the caret) with one `ReplaceText` command, so it is one
//! undo step. Input methods (Japanese, Chinese, Korean, Indic keyboards, dead keys) first show
//! *preedit* text: a draft, underlined, that changes with each key until the user commits it. The
//! draft lives only here in the editor and is drawn over the document, never written to it, so a
//! composition makes no undo steps, no saves and nothing for other devices until it is committed.
//! An empty draft is the same as none: it never deletes the selection (ADR 0017).

use leaf_model::{Caret, PageCommand};

use crate::caret::motion;
use crate::format::apply_pending;
use crate::selection::delete_span;
use crate::{EditorError, PageEditor, TextRange};

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
    /// Types `text`: replaces the selection, or inserts at the caret, as one undo step, and puts
    /// the caret after it. If that completes a Markdown shortcut at the start of the block (`# `,
    /// `- `, …), the block is converted, as an undo step of its own. A selection across blocks is deleted first (joining what is left), in the
    /// same step. Empty text, no caret, a block without text (a divider), or whole blocks selected
    /// change nothing.
    pub fn insert_text(&mut self, text: &str) -> Result<Option<Caret>, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        if text.is_empty() || self.blocks.is_some() || self.doc.plain_text(caret.block)?.is_none() {
            return Ok(Some(caret));
        }
        // Undo puts the selection back as it was before the typing.
        self.begin()?;
        let (start, end) = self.text_selection()?.unwrap_or((caret, caret));
        let pending = self.pending_at(caret).unwrap_or_default();
        let typed = text.chars().count();
        let order = self.order()?;
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            if start.block == end.block {
                doc.apply(PageCommand::ReplaceText {
                    id: start.block,
                    range: start.at..end.at,
                    text: text.to_owned(),
                })?;
            } else {
                let at = delete_span(doc, &order, start, end)?;
                doc.apply(PageCommand::InsertText {
                    id: at.block,
                    pos: at.at,
                    text: text.to_owned(),
                })?;
            }
            apply_pending(doc, start.block, start.at..start.at + typed, &pending)
        })?;
        let after = self.set_caret(Caret {
            at: start.at + typed,
            ..start
        })?;
        if !pending.is_empty() {
            self.carry_pending(pending, after);
        }
        self.finish()?;
        if let Some(converted) = self.markdown_shortcut(after)? {
            return Ok(Some(converted));
        }
        self.maybe_open_slash(text, after)?;
        self.maybe_open_link_menu(text, after)?;
        Ok(Some(after))
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
        // Across blocks the draft shows where the selection starts; the commit replaces all of it.
        let at = match self.text_selection()? {
            Some((start, end)) if start.block == end.block => TextRange {
                block: start.block,
                range: start.at..end.at,
            },
            Some((start, _)) => TextRange {
                block: start.block,
                range: start.at..start.at,
            },
            None => TextRange {
                block: caret.block,
                range: caret.at..caret.at,
            },
        };
        Ok(Some(Preedit { at, text, cursor }))
    }
}

#[cfg(test)]
mod tests;
