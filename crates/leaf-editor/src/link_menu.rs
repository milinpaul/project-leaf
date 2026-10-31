//! The page-link menu's typing side (E10-S05, ADR-071): typing `[[` opens it, and what follows is the
//! query. Like the slash menu, `[[` and the query are ordinary text in the page, so they stay if the menu
//! closes without a choice; the editor only remembers where the `[[` is and which entry is highlighted.
//! Which pages match is the workspace's business: see [`WorkspaceEditor::link_menu`](crate::WorkspaceEditor::link_menu).

use leaf_core::PageId;
use leaf_model::{Anchor, Caret, PageCommand};

use crate::{EditorError, PageEditor};

/// What opens the menu.
const OPEN: &str = "[[";

/// The open menu as the editor keeps it.
pub(crate) struct LinkMenuState {
    /// Where the `[[` is.
    at: Anchor,
    /// The highlighted entry.
    pub(crate) selected: usize,
    /// Opened by "Link to page" over this many selected characters, which became the `[[` and query in a
    /// step of their own: Escape undoes that step, giving the words back, and choosing a page undoes it
    /// before swapping the words for the link, so one undo gives the words back.
    from_selection: Option<usize>,
    /// The query the highlight belongs to; a new query moves the highlight back to the top.
    query: String,
}

impl PageEditor {
    /// Where the open menu's `[[` is and what was typed after it, if the menu is open. It closes by
    /// itself when the `[[` is deleted, the caret leaves the query, or the query gets a `]` or a line
    /// break. A changed query moves the highlight back to the top.
    pub fn link_query(&mut self) -> Result<Option<(Caret, String)>, EditorError> {
        let Some(menu) = &self.link_menu else {
            return Ok(None);
        };
        let anchor = menu.at.clone();
        let (Some(at), Some(caret)) = (self.doc.resolve(&anchor)?, self.caret()?) else {
            self.link_menu = None;
            return Ok(None);
        };
        let text: Vec<char> = self
            .doc
            .plain_text(at.block)?
            .unwrap_or_default()
            .chars()
            .collect();
        let start = at.at + OPEN.len();
        let opened = text.get(at.at..start).is_some_and(|two| two == ['[', '[']);
        let inside = caret.block == at.block && caret.at >= start && caret.at <= text.len();
        let query: String = if opened && inside {
            text[start..caret.at].iter().collect()
        } else {
            String::new()
        };
        if !opened || !inside || query.contains([']', '\n']) || self.selection()?.is_some() {
            self.link_menu = None;
            return Ok(None);
        }
        if let Some(menu) = &mut self.link_menu
            && menu.query != query
        {
            menu.query.clone_from(&query);
            menu.selected = 0;
        }
        Ok(Some((at, query)))
    }

    /// Escape: closes the menu, leaving the typed `[[` and query as text.
    pub fn link_menu_close(&mut self) {
        self.link_menu = None;
    }

    /// Escape with the menu open: a menu opened from selected words gives them back as they were (the
    /// step that made them a query is undone); one opened by typing just closes. Returns whether a menu
    /// was open.
    pub fn link_menu_escape(&mut self) -> Result<bool, EditorError> {
        if self.link_query()?.is_none() {
            return Ok(false);
        }
        let from_selection = self.link_menu.as_ref().and_then(|menu| menu.from_selection);
        self.link_menu = None;
        if from_selection.is_some() {
            self.undo()?;
        }
        Ok(true)
    }

    /// "Link to page" over selected words (E10-S10): they become a `[[` query in a step of their own and the
    /// menu opens, searching for them; choosing a page swaps them for its link. `None` when the selection
    /// can't search the list (see [`FormatBar::link_to_page`](crate::FormatBar)).
    pub fn link_selection_to_page(&mut self) -> Result<Option<Caret>, EditorError> {
        let Some(crate::Selection::Text { start, end }) = self.selection()? else {
            return Ok(None);
        };
        if !self.selection_can_link(start, end)? {
            return Ok(None);
        }
        let (block, from, to) = (start.block, start.at.min(end.at), start.at.max(end.at));
        let words: String = self
            .doc
            .plain_text(block)?
            .unwrap_or_default()
            .chars()
            .skip(from)
            .take(to - from)
            .collect();
        self.begin()?;
        let query = format!("{OPEN}{words}");
        self.doc.as_separate_step(|doc| {
            doc.apply(PageCommand::ReplaceText {
                id: block,
                range: from..to,
                text: query.clone(),
            })
        })?;
        let caret = self.set_caret(Caret {
            block,
            at: from + query.chars().count(),
        })?;
        self.finish()?;
        self.open_link_menu(Caret { block, at: from })?;
        if let Some(menu) = &mut self.link_menu {
            menu.from_selection = Some(to - from);
        }
        Ok(Some(caret))
    }

    /// Whether the text from `start` to `end` can become a page-list search: one block that takes links,
    /// some words, no line break, `]` or page link.
    pub(crate) fn selection_can_link(&self, start: Caret, end: Caret) -> Result<bool, EditorError> {
        if start.block != end.block || start.at == end.at || !self.doc.kind(start.block)?.allows_marks() {
            return Ok(false);
        }
        let (from, to) = (start.at.min(end.at), start.at.max(end.at));
        let words: String = self
            .doc
            .plain_text(start.block)?
            .unwrap_or_default()
            .chars()
            .skip(from)
            .take(to - from)
            .collect();
        Ok(!words.trim().is_empty() && !words.contains([']', '\n', leaf_model::PAGE_LINK]))
    }

    /// Replaces the `[[` and the query with a link to `page`, the caret right after it, as one undo step
    /// of its own (undo right after gives back the typed text). `None` when the menu isn't open.
    pub fn complete_link(&mut self, page: PageId) -> Result<Option<Caret>, EditorError> {
        let Some((at, query)) = self.link_query()? else {
            return Ok(None);
        };
        let from_selection = self.link_menu.as_ref().and_then(|menu| menu.from_selection);
        self.link_menu = None;
        let typed = match from_selection {
            // Back to the selected words first, so the link replaces them in one step.
            Some(words) => {
                self.undo()?;
                at.at..at.at + words
            }
            None => at.at..at.at + OPEN.len() + query.chars().count(),
        };
        self.begin()?;
        self.doc.as_separate_step(|doc| -> Result<(), EditorError> {
            doc.apply(PageCommand::DeleteText {
                id: at.block,
                range: typed.clone(),
            })?;
            doc.apply(PageCommand::InsertPageLink {
                id: at.block,
                pos: at.at,
                page,
            })?;
            Ok(())
        })?;
        let caret = self.set_caret(Caret {
            block: at.block,
            at: at.at + 1,
        })?;
        self.finish()?;
        Ok(Some(caret))
    }

    /// After typing that left the caret at `caret`: opens the menu if the typing ended a `[[` in a block
    /// that takes links (not code, which takes no marks).
    pub(crate) fn maybe_open_link_menu(&mut self, typed: &str, caret: Caret) -> Result<(), EditorError> {
        if !typed.ends_with('[') || !self.doc.kind(caret.block)?.allows_marks() {
            return Ok(());
        }
        let Some(start) = caret.at.checked_sub(OPEN.len()) else {
            return Ok(());
        };
        let text = self.doc.plain_text(caret.block)?.unwrap_or_default();
        let two: String = text.chars().skip(start).take(OPEN.len()).collect();
        if two == OPEN {
            self.open_link_menu(Caret {
                block: caret.block,
                at: start,
            })?;
        }
        Ok(())
    }

    /// Opens the menu for a `[[` that starts at `at` (the slash menu's "Link to page" types one).
    pub(crate) fn open_link_menu(&mut self, at: Caret) -> Result<(), EditorError> {
        self.slash = None;
        self.link_menu = Some(LinkMenuState {
            at: self.doc.anchor(at)?,
            selected: 0,
            query: String::new(),
            from_selection: None,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
