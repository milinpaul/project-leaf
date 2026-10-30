//! Inline formatting from the keyboard: Cmd/Ctrl+B, I, E and Shift+S toggles, and links.
//!
//! A toggle looks at the whole selection: if every selected character already has the mark, it is
//! removed; otherwise it is applied to all of them. Code blocks take no marks and are left out, both
//! when deciding and when applying. With no selection, a toggle changes what the *next typed text*
//! gets instead, until the caret moves elsewhere (ADR 0022).

use std::ops::Range;

use leaf_core::BlockId;
use leaf_model::{Caret, Mark, MarkKind, Marks, PageCommand, PageDoc};

use crate::{EditorError, PageEditor, Selection, TextRange};

/// Marks to set (`true`) or clear (`false`) on the next typed text, at one caret position.
pub(crate) struct Pending {
    at: Caret,
    marks: Vec<(MarkKind, bool)>,
}

impl PageEditor {
    /// Cmd/Ctrl+B ([`MarkKind::Bold`]), I, E (code), Shift+S (strike). Over a selection (text or whole
    /// blocks), adds the mark to all of it unless all of it already has it, in which case it is removed;
    /// one undo step. With just a caret, flips the mark for the next typed text. For
    /// [`MarkKind::Link`], removes the link if the selection is all linked (a link needs an address to
    /// be added: [`PageEditor::set_link`]).
    pub fn toggle_mark(&mut self, kind: MarkKind) -> Result<(), EditorError> {
        let ranges = self.markable_ranges()?;
        if ranges.is_empty() {
            if kind != MarkKind::Link
                && self.selection()?.is_none()
                && let Some(caret) = self.caret()?
            {
                let on = !has(&self.typing_marks()?, kind);
                let mut marks = self.pending_at(caret).unwrap_or_default();
                marks.retain(|(k, _)| *k != kind);
                marks.push((kind, on));
                self.pending = Some(Pending { at: caret, marks });
            }
            return Ok(());
        }
        let all = ranges
            .iter()
            .map(|r| covered(&self.doc, r, kind))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .all(|b| b);
        if kind == MarkKind::Link && !all {
            return Ok(());
        }
        self.begin()?;
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            for TextRange { block, range } in ranges {
                let command = if all {
                    PageCommand::RemoveMark {
                        id: block,
                        range,
                        kind,
                    }
                } else {
                    PageCommand::SetMark {
                        id: block,
                        range,
                        mark: mark_of(kind),
                    }
                };
                doc.apply(command)?;
            }
            Ok(())
        })?;
        self.finish()
    }

    /// Cmd/Ctrl+K's popover saved an address. Over a selection, links all of it to `href`; with just a
    /// caret inside a link, changes that whole link's address. A blank address removes the link.
    pub fn set_link(&mut self, href: &str) -> Result<(), EditorError> {
        let href = href.trim();
        if href.is_empty() {
            return self.remove_link();
        }
        let Some(ranges) = self.link_or_selection()? else {
            return Ok(());
        };
        self.begin()?;
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            for TextRange { block, range } in ranges {
                doc.apply(PageCommand::SetMark {
                    id: block,
                    range,
                    mark: Mark::Link(href.to_owned()),
                })?;
            }
            Ok(())
        })?;
        self.finish()
    }

    /// Removes the link from the selection, or from the whole link the caret is in. One undo step.
    pub fn remove_link(&mut self) -> Result<(), EditorError> {
        let Some(ranges) = self.link_or_selection()? else {
            return Ok(());
        };
        self.begin()?;
        self.doc.as_one_step(|doc| -> Result<(), EditorError> {
            for TextRange { block, range } in ranges {
                doc.apply(PageCommand::RemoveMark {
                    id: block,
                    range,
                    kind: MarkKind::Link,
                })?;
            }
            Ok(())
        })?;
        self.finish()
    }

    /// The link the caret is in, with its whole extent and address, for the link popover. `None`
    /// when the caret isn't inside a link, or text is selected.
    pub fn link_at_caret(&mut self) -> Result<Option<(TextRange, String)>, EditorError> {
        if self.selection()?.is_some() {
            return Ok(None);
        }
        let Some(caret) = self.caret()? else {
            return Ok(None);
        };
        let marks = char_marks(&self.doc, caret.block)?;
        // The caret is in a link when a linked character touches it; prefer the one after.
        let touching = [Some(caret.at), caret.at.checked_sub(1)]
            .into_iter()
            .flatten()
            .find_map(|i| marks.get(i).and_then(|m| m.link.clone()).map(|href| (i, href)));
        let Some((i, href)) = touching else {
            return Ok(None);
        };
        let same = |j: usize| marks[j].link.as_deref() == Some(href.as_str());
        let start = (0..=i).rev().take_while(|&j| same(j)).last().unwrap_or(i);
        let end = (i..marks.len())
            .take_while(|&j| same(j))
            .last()
            .map_or(i, |j| j + 1);
        Ok(Some((
            TextRange {
                block: caret.block,
                range: start..end,
            },
            href,
        )))
    }

    /// The formatting toolbar's state: the marks every selected character shares, or, with just a
    /// caret, the marks the next typed text will get.
    pub fn active_marks(&mut self) -> Result<Marks, EditorError> {
        let ranges = self.markable_ranges()?;
        if ranges.is_empty() {
            return self.typing_marks();
        }
        let mut shared: Option<Marks> = None;
        for TextRange { block, range } in ranges {
            for marks in &char_marks(&self.doc, block)?[range] {
                shared = Some(match shared {
                    None => marks.clone(),
                    Some(s) => Marks {
                        bold: s.bold && marks.bold,
                        italic: s.italic && marks.italic,
                        strike: s.strike && marks.strike,
                        code: s.code && marks.code,
                        link: s.link.filter(|l| marks.link.as_ref() == Some(l)),
                    },
                });
            }
        }
        Ok(shared.unwrap_or_default())
    }

    /// What typing at the caret produces: the mark table's expand rules (bold, italic and strike
    /// carry on from the character before; code and links only inside them), then any pending toggles.
    fn typing_marks(&mut self) -> Result<Marks, EditorError> {
        let Some(caret) = self.caret()? else {
            return Ok(Marks::default());
        };
        let marks = char_marks(&self.doc, caret.block)?;
        let before = caret
            .at
            .checked_sub(1)
            .and_then(|i| marks.get(i))
            .cloned()
            .unwrap_or_default();
        let after = marks.get(caret.at).cloned().unwrap_or_default();
        let mut typed = Marks {
            bold: before.bold,
            italic: before.italic,
            strike: before.strike,
            code: before.code && after.code,
            link: before.link.filter(|l| after.link.as_ref() == Some(l)),
        };
        for (kind, on) in self.pending_at(caret).unwrap_or_default() {
            match kind {
                MarkKind::Bold => typed.bold = on,
                MarkKind::Italic => typed.italic = on,
                MarkKind::Strike => typed.strike = on,
                MarkKind::Code => typed.code = on,
                MarkKind::Link => {}
            }
        }
        Ok(typed)
    }

    /// The pending toggles, if they were made at `caret`; moving the caret anywhere else drops them.
    pub(crate) fn pending_at(&self, caret: Caret) -> Option<Vec<(MarkKind, bool)>> {
        self.pending
            .as_ref()
            .filter(|p| p.at == caret)
            .map(|p| p.marks.clone())
    }

    /// Keeps pending toggles alive while typing moves the caret along.
    pub(crate) fn carry_pending(&mut self, marks: Vec<(MarkKind, bool)>, at: Caret) {
        self.pending = Some(Pending { at, marks });
    }

    /// The selection's text in each block that can take marks, as non-empty ranges in reading order.
    pub(crate) fn markable_ranges(&mut self) -> Result<Vec<TextRange>, EditorError> {
        let mut pieces: Vec<(BlockId, Range<usize>)> = Vec::new();
        match self.selection()? {
            None => {}
            Some(Selection::Text { start, end }) => {
                for block in self.blocks_between(start, end)? {
                    let len = text_len(&self.doc, block)?;
                    let from = if block == start.block { start.at } else { 0 };
                    let to = if block == end.block { end.at } else { len };
                    pieces.push((block, from..to));
                }
            }
            Some(Selection::Blocks(blocks)) => {
                for block in self.with_descendants(&blocks)? {
                    pieces.push((block, 0..text_len(&self.doc, block)?));
                }
            }
            Some(Selection::Cells { cells, .. }) => {
                for block in cells {
                    pieces.push((block, 0..text_len(&self.doc, block)?));
                }
            }
        }
        let mut ranges = Vec::new();
        for (block, range) in pieces {
            if !range.is_empty() && self.doc.kind(block)?.allows_marks() {
                ranges.push(TextRange { block, range });
            }
        }
        Ok(ranges)
    }

    /// The selection, or else the whole link the caret is in.
    fn link_or_selection(&mut self) -> Result<Option<Vec<TextRange>>, EditorError> {
        if self.selection()?.is_some() {
            let ranges = self.markable_ranges()?;
            return Ok((!ranges.is_empty()).then_some(ranges));
        }
        Ok(self.link_at_caret()?.map(|(range, _)| vec![range]))
    }

    /// `blocks` and everything nested in them, in reading order.
    fn with_descendants(&self, blocks: &[BlockId]) -> Result<Vec<BlockId>, EditorError> {
        let mut out = Vec::new();
        let mut stack: Vec<BlockId> = blocks.iter().rev().copied().collect();
        while let Some(block) = stack.pop() {
            out.push(block);
            stack.extend(self.doc.children(block)?.into_iter().rev());
        }
        Ok(out)
    }
}

/// Applies pending toggles to text just typed at `range` in `block`, inside the typing's undo step.
pub(crate) fn apply_pending(
    doc: &mut PageDoc,
    block: BlockId,
    range: Range<usize>,
    pending: &[(MarkKind, bool)],
) -> Result<(), EditorError> {
    if !doc.kind(block)?.allows_marks() {
        return Ok(());
    }
    for &(kind, on) in pending {
        let range = range.clone();
        doc.apply(if on {
            PageCommand::SetMark {
                id: block,
                range,
                mark: mark_of(kind),
            }
        } else {
            PageCommand::RemoveMark {
                id: block,
                range,
                kind,
            }
        })?;
    }
    Ok(())
}

/// The mark for a toggle. Links are never toggled on (they need an address), so `Link` never gets here.
fn mark_of(kind: MarkKind) -> Mark {
    match kind {
        MarkKind::Bold => Mark::Bold,
        MarkKind::Italic => Mark::Italic,
        MarkKind::Strike => Mark::Strike,
        MarkKind::Code | MarkKind::Link => Mark::Code,
    }
}

fn has(marks: &Marks, kind: MarkKind) -> bool {
    match kind {
        MarkKind::Bold => marks.bold,
        MarkKind::Italic => marks.italic,
        MarkKind::Strike => marks.strike,
        MarkKind::Code => marks.code,
        MarkKind::Link => marks.link.is_some(),
    }
}

/// Whether every character in `range` has the mark.
fn covered(doc: &PageDoc, range: &TextRange, kind: MarkKind) -> Result<bool, EditorError> {
    Ok(char_marks(doc, range.block)?[range.range.clone()]
        .iter()
        .all(|m| has(m, kind)))
}

/// The marks of each character of the block's text, by Unicode scalar position.
pub(crate) fn char_marks(doc: &PageDoc, block: BlockId) -> Result<Vec<Marks>, EditorError> {
    Ok(doc
        .text(block)?
        .unwrap_or_default()
        .into_iter()
        .flat_map(|span| std::iter::repeat_n(span.marks, span.text.chars().count()))
        .collect())
}

fn text_len(doc: &PageDoc, block: BlockId) -> Result<usize, EditorError> {
    Ok(doc.plain_text(block)?.map_or(0, |t| t.chars().count()))
}

#[cfg(test)]
mod tests;
