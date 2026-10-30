//! A page's menu (E18-S04): the actions in the top bar's `⋯` and in the sidebar's right-click menu,
//! from one list, so both offer the same things in the same order and a later action is one more
//! entry here rather than a new menu. The UI draws the entries and runs the chosen action.

use leaf_core::PageId;

use super::{EditorError, WorkspaceEditor};
use crate::edited::{edited_ago, edited_on};

/// What a page menu entry does. The UI carries each one out (renaming, for one, is drawn differently
/// in the sidebar and the top bar), through the same editor calls wherever the menu was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageAction {
    /// A new page inside this one (sidebar only: the top bar's page is already open).
    AddInside,
    /// Star or unstar the page.
    ToggleFavourite,
    /// Edit the title.
    Rename,
    /// Switch the page between the reading column and the whole width (top bar only).
    ToggleFullWidth,
    /// Move the page, with everything inside it, to the trash.
    MoveToTrash,
}

/// The picture beside an entry; the UI maps each to an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMenuIcon {
    /// "+".
    Add,
    /// An outline star (add to Favourites).
    Star,
    /// A crossed-out star (remove from Favourites).
    StarOff,
    /// A pencil.
    Rename,
    /// Arrows pointing out to both sides.
    FullWidth,
    /// A bin.
    Trash,
}

/// Where the menu was opened, which decides a few entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPlace {
    /// The `⋯` button in the top bar of the open page: with a footer of its times.
    TopBar,
    /// A sidebar row's right-click menu: with "Add page inside".
    Sidebar,
}

/// One entry of a page menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMenuEntry {
    /// What it does.
    pub action: PageAction,
    /// What it says.
    pub label: &'static str,
    /// Its picture.
    pub icon: PageMenuIcon,
    /// Its keyboard shortcut to show, if it has one.
    pub shortcut: Option<&'static str>,
    /// Drawn in red: it takes something away.
    pub danger: bool,
    /// Whether it can be chosen now.
    pub enabled: bool,
    /// Whether a line separates it from the entry above.
    pub separated: bool,
    /// For an entry that turns something on or off: whether it is on now, so the UI can draw a switch.
    /// `None` for an entry that just does something.
    pub switch: Option<bool>,
}

/// A page's menu: its entries, then lines of information under them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMenu {
    /// The entries, in order.
    pub entries: Vec<PageMenuEntry>,
    /// Lines shown under the entries, such as when the page was created.
    pub footer: Vec<String>,
}

impl WorkspaceEditor {
    /// The menu for `page` opened at `place`. `now` (Unix milliseconds) and `offset_minutes` (the
    /// local time zone's offset from UTC) come from the UI, for the footer's times.
    pub fn page_menu(
        &self,
        page: PageId,
        place: MenuPlace,
        now: i64,
        offset_minutes: i32,
    ) -> Result<PageMenu, EditorError> {
        let meta = self.workspace.page(page)?;
        let favourite = self.is_favourite(page);
        let entry = |action, label, icon| PageMenuEntry {
            action,
            label,
            icon,
            shortcut: None,
            danger: false,
            enabled: true,
            separated: false,
            switch: None,
        };
        let mut entries = Vec::new();
        if place == MenuPlace::Sidebar {
            entries.push(entry(PageAction::AddInside, "Add page inside", PageMenuIcon::Add));
        }
        entries.push(if favourite {
            entry(
                PageAction::ToggleFavourite,
                "Remove from Favourites",
                PageMenuIcon::StarOff,
            )
        } else {
            entry(
                PageAction::ToggleFavourite,
                "Add to Favourites",
                PageMenuIcon::Star,
            )
        });
        entries.push(entry(PageAction::Rename, "Rename", PageMenuIcon::Rename));
        // How the open page is laid out: only where the page is open, like Notion.
        if place == MenuPlace::TopBar {
            entries.push(PageMenuEntry {
                separated: true,
                switch: Some(meta.full_width),
                ..entry(PageAction::ToggleFullWidth, "Full width", PageMenuIcon::FullWidth)
            });
        }
        entries.push(PageMenuEntry {
            danger: true,
            separated: true,
            ..entry(PageAction::MoveToTrash, "Move to Trash", PageMenuIcon::Trash)
        });
        let footer = match place {
            MenuPlace::TopBar => vec![
                format!("Created {}", edited_on(meta.created_at, offset_minutes)),
                format!("Last {}", edited_ago(now, meta.edited_at).to_lowercase()),
            ],
            MenuPlace::Sidebar => Vec::new(),
        };
        Ok(PageMenu { entries, footer })
    }

    /// "Move to Trash" from a page menu: trashes `page` (with everything inside it) and, if the open
    /// page went with it, opens the nearest page still there: its parent, else the page after it,
    /// else the one before, else the first page. Returns the page opened instead, if any.
    pub fn trash_and_open_next(&mut self, page: PageId, now: i64) -> Result<Option<PageId>, EditorError> {
        let parent = self.workspace.parent(page)?;
        let siblings = self.workspace.child_slots(parent)?;
        let at = siblings.iter().position(|sibling| *sibling == page);
        let was_open = self.selected;
        self.trash(page, now)?;
        if was_open.is_none() || self.selected.is_some() {
            return Ok(None);
        }
        let after = at.and_then(|at| siblings.get(at + 1).copied());
        let before = at
            .and_then(|at| at.checked_sub(1))
            .and_then(|at| siblings.get(at).copied());
        let first = self.workspace.root_pages()?.first().copied();
        let next = [parent, after, before, first]
            .into_iter()
            .flatten()
            .find(|candidate| !self.workspace.is_in_trash(*candidate).unwrap_or(true));
        if let Some(next) = next {
            self.select(next)?;
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests;
