//! The page-link menu as drawn (E10-S05, ADR 0071): the open page's `[[` query matched against the
//! workspace's pages with the quick switcher's fuzzy matcher, plus "New page" when no title is exactly
//! the query. Choosing an entry links it, creating the page first for "New page".

use leaf_core::PageId;
use leaf_model::{Caret, Outcome, WorkspaceCommand};

use super::{EditorError, WorkspaceEditor};
use crate::caret::Vertical;
use crate::switcher::{QuickSwitcher, SwitcherResult};

/// How many pages the menu lists at most: a short list by the caret, not a full search.
pub const MAX_LINK_CHOICES: usize = 8;

/// One entry of the page-link menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkChoice {
    /// An existing page, with its icon, title, where it lives and what matched.
    Page(SwitcherResult),
    /// A new page with this title, made inside the open page.
    NewPage(String),
}

/// The open page-link menu, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkMenu {
    /// Where the `[[` is, to place the menu under it.
    pub at: Caret,
    /// What was typed after the `[[`.
    pub query: String,
    /// The entries, best first; "New page" last.
    pub choices: Vec<LinkChoice>,
    /// Which entry is highlighted.
    pub selected: usize,
}

impl WorkspaceEditor {
    /// The open page's page-link menu, if one is open. Pages in the trash, and the open page itself, are
    /// not offered.
    pub fn link_menu(&mut self) -> Result<Option<LinkMenu>, EditorError> {
        let Some(current) = self.selected() else {
            return Ok(None);
        };
        let Some(page) = self.open_page()? else {
            return Ok(None);
        };
        let Some((at, query)) = page.link_query()? else {
            return Ok(None);
        };
        let selected = page.link_menu.as_ref().map_or(0, |menu| menu.selected);
        let mut switcher = QuickSwitcher::open(self.workspace(), self.recent_pages())?;
        switcher.set_query(query.trim());
        let mut choices: Vec<LinkChoice> = switcher
            .results()
            .iter()
            .filter(|result| result.page != current)
            .take(MAX_LINK_CHOICES)
            .cloned()
            .map(LinkChoice::Page)
            .collect();
        let title = query.trim();
        let exact = choices.iter().any(|choice| {
            matches!(choice, LinkChoice::Page(result) if result.label.to_lowercase() == title.to_lowercase())
        });
        if !title.is_empty() && !exact {
            choices.push(LinkChoice::NewPage(title.to_owned()));
        }
        let selected = selected.min(choices.len().saturating_sub(1));
        Ok(Some(LinkMenu {
            at,
            query,
            choices,
            selected,
        }))
    }

    /// Up and Down while the menu is open: moves the highlight, wrapping around. Returns the menu.
    pub fn link_menu_move(&mut self, direction: Vertical) -> Result<Option<LinkMenu>, EditorError> {
        let Some(menu) = self.link_menu()? else {
            return Ok(None);
        };
        let count = menu.choices.len();
        if count > 0 {
            let next = match direction {
                Vertical::Up => (menu.selected + count - 1) % count,
                Vertical::Down => (menu.selected + 1) % count,
            };
            self.set_link_selected(next)?;
        }
        self.link_menu()
    }

    /// The mouse over entry `index`: highlights it. Returns the menu.
    pub fn link_menu_highlight(&mut self, index: usize) -> Result<Option<LinkMenu>, EditorError> {
        if self.link_menu()?.is_some_and(|menu| index < menu.choices.len()) {
            self.set_link_selected(index)?;
        }
        self.link_menu()
    }

    /// Enter, or a click on entry `index` (`None` for the highlighted one): links the chosen page in
    /// place of the `[[` and query; "New page" first creates it, titled, inside the open page (without
    /// opening it). Returns where the caret went, or `None` with no menu or nothing to choose.
    pub fn link_menu_apply(
        &mut self,
        index: Option<usize>,
        now_ms: i64,
    ) -> Result<Option<Caret>, EditorError> {
        let Some(menu) = self.link_menu()? else {
            return Ok(None);
        };
        let Some(choice) = menu.choices.get(index.unwrap_or(menu.selected)).cloned() else {
            return Ok(None);
        };
        let target = match choice {
            LinkChoice::Page(result) => result.page,
            LinkChoice::NewPage(title) => self.new_linked_page(&title, now_ms)?,
        };
        match self.open_page()? {
            Some(page) => page.complete_link(target),
            None => Ok(None),
        }
    }

    /// A page titled `title` as the last child of the open page, which opens in the sidebar so it shows;
    /// the open page stays open.
    fn new_linked_page(&mut self, title: &str, now_ms: i64) -> Result<PageId, EditorError> {
        let parent = self.selected();
        let index = self.workspace.child_slots(parent)?.len();
        let Outcome::PageCreated(page) = self.workspace.apply(WorkspaceCommand::CreatePage {
            parent,
            index,
            title: title.to_owned(),
            created_at: now_ms,
        })?
        else {
            unreachable!("CreatePage always reports the page it created");
        };
        if let Some(parent) = parent {
            self.set_expanded(parent, true);
        }
        Ok(page)
    }

    fn set_link_selected(&mut self, index: usize) -> Result<(), EditorError> {
        if let Some(menu) = self.open_page()?.and_then(|page| page.link_menu.as_mut()) {
            menu.selected = index;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
