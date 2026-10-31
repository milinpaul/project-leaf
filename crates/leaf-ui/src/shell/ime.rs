//! The platform's text input for the page: typed characters, input methods (Japanese, Chinese, Korean,
//! Hindi and other keyboards that compose) and the emoji picker all arrive here (ADR-040).
//!
//! The input method works on one text: the caret's block, with the editor's draft (preedit, ADR-017)
//! shown in place of what it will replace. The draft lives only in the editor until committed, so
//! this "virtual" text is what the view paints and what positions are measured on; offsets are in
//! UTF-16, as platforms count them.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, px, size};
use leaf_core::BlockId;

use super::Shell;
use super::rich_text::with_draft;

/// The caret block's text as the input method sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImeText {
    /// The block.
    pub block: BlockId,
    /// Its text with the draft (if any) in place of what it covers.
    pub text: String,
    /// The selection, or the caret as an empty range, in characters.
    pub selected: Range<usize>,
    /// Whether the caret is at the selection's start (it was made right to left).
    pub reversed: bool,
    /// Where the draft is, in characters.
    pub marked: Option<Range<usize>>,
}

impl Shell {
    /// The caret block's text as the input method sees it, if the page has a caret in a text block.
    pub(super) fn ime_text(&mut self) -> Option<ImeText> {
        let page = self.page()?;
        let caret = page.caret().ok().flatten()?;
        let spans = page.doc().text(caret.block).ok().flatten()?;
        if let Some(draft) = page.preedit().ok().flatten() {
            let (spans, marked) = with_draft(&spans, &draft.at.range, &draft.text);
            let at = marked.start + draft.cursor;
            return Some(ImeText {
                block: caret.block,
                text: spans.iter().map(|s| s.text.as_str()).collect(),
                selected: at..at,
                reversed: false,
                marked: Some(marked),
            });
        }
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let (selected, reversed) = match page.selection().ok().flatten() {
            Some(leaf_editor::Selection::Text { start, end })
                if start.block == end.block && start.block == caret.block =>
            {
                (start.at..end.at, caret.at == start.at)
            }
            _ => (caret.at..caret.at, false),
        };
        Some(ImeText {
            block: caret.block,
            text,
            selected,
            reversed,
            marked: None,
        })
    }

    /// After the input method changed something: redraw and tell the platform where things moved.
    fn ime_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.restart_blink();
        self.reveal_caret();
        window.invalidate_character_coordinates();
        cx.notify();
    }
}

impl EntityInputHandler for Shell {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let ime = self.ime_text()?;
        let chars = from_utf16(&ime.text, &range);
        adjusted.replace(to_utf16(&ime.text, &chars));
        Some(ime.text.chars().skip(chars.start).take(chars.len()).collect())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let ime = self.ime_text()?;
        Some(UTF16Selection {
            range: to_utf16(&ime.text, &ime.selected),
            reversed: ime.reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let draft = self.blocks.draft.as_ref()?;
        // As drawn this frame; ime_text needs `&mut self`, which this method doesn't get.
        let start: usize = draft.at.range.start;
        let text: String = self
            .blocks
            .list
            .rows
            .iter()
            .find(|row| row.id == draft.at.block)
            .and_then(|row| row.text.clone())
            .unwrap_or_default();
        let before: usize = text.chars().take(start).map(char::len_utf16).sum();
        let len: usize = draft.text.chars().map(char::len_utf16).sum();
        Some(before..before + len)
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The composition ended without the input method committing (focus moved away): keep what
        // the user saw. macOS also calls this before every click reaches the page, so it never
        // scrolls: revealing the old caret here jumped the page back to it (E17-S06).
        if let Some(page) = self.page()
            && let Some(draft) = page.preedit().ok().flatten()
        {
            let _ = page.commit_preedit(&draft.text);
            self.restart_blink();
            window.invalidate_character_coordinates();
            cx.notify();
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ime = self.ime_text();
        let Some(page) = self.page() else { return };
        let result = if ime.as_ref().is_some_and(|ime| ime.marked.is_some()) {
            // Committing a composition: it replaces what the draft covered.
            page.commit_preedit(text).map(|_| ())
        } else {
            if let (Some(range), Some(ime)) = (range, &ime) {
                // Replacing a given range (an accent menu, a correction): select it first.
                let chars = from_utf16(&ime.text, &range);
                let _ = page.select(ime.block, chars);
            }
            page.insert_text(text).map(|_| ())
        };
        if let Err(err) = result {
            super::report("could not type", err);
        }
        self.ime_changed(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ime = self.ime_text();
        let Some(page) = self.page() else { return };
        // A composition starting over existing text (reconversion) covers that text.
        if let (Some(range), Some(ime)) = (range, &ime)
            && ime.marked.is_none()
        {
            let _ = page.select(ime.block, from_utf16(&ime.text, &range));
        }
        let cursor = selected.map_or_else(
            || text.chars().count(),
            |sel| from_utf16(text, &(sel.end..sel.end)).end,
        );
        page.set_preedit(text, cursor);
        self.ime_changed(window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let ime = self.ime_text()?;
        let chars = from_utf16(&ime.text, &range);
        let (start, height) = self.blocks.painted.caret_bounds(ime.block, chars.start)?;
        let end = self
            .blocks
            .painted
            .caret_bounds(ime.block, chars.end)
            .map_or(start.x, |(end, _)| end.x);
        Some(Bounds::new(start, size((end - start.x).max(px(2.0)), height)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let ime = self.ime_text()?;
        let at = self.blocks.painted.offset_at(ime.block, point)?;
        Some(to_utf16(&ime.text, &(at..at)).start)
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        self.ime_text().map(|ime| ime.text.encode_utf16().count())
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        self.blocks.caret.is_some()
    }
}

/// A UTF-16 range of `text` as a character range, clamped to the text.
pub(super) fn from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    let mut chars = 0;
    let mut units = 0;
    let (mut start, mut end) = (None, None);
    for c in text.chars() {
        if start.is_none() && units >= range.start {
            start = Some(chars);
        }
        if end.is_none() && units >= range.end {
            end = Some(chars);
        }
        units += c.len_utf16();
        chars += 1;
    }
    let start = start.unwrap_or(chars);
    let end = end.unwrap_or(chars).max(start);
    start..end
}

/// A character range of `text` in UTF-16 units.
pub(super) fn to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    let units = |n: usize| -> usize { text.chars().take(n).map(char::len_utf16).sum() };
    units(range.start)..units(range.end)
}

#[cfg(test)]
mod tests;
