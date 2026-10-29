//! Editing the workspace: which page is selected, what the sidebar shows, and the page actions the
//! sidebar offers. Turns those actions into `WorkspaceCommand`s; never touches Loro itself.
//!
//! The page tree itself (rows, expanding, moving, trashing) is in `tree`.

use std::collections::{HashMap, HashSet};

use leaf_core::PageId;
use leaf_model::{ModelError, PageDoc, Workspace, WorkspaceCommand};

use crate::PageEditor;
use thiserror::Error;

/// How many recently opened pages are remembered.
pub const RECENT_LIMIT: usize = 20;

/// What the sidebar shows for an empty title, so a new page is never an invisible row.
pub const UNTITLED: &str = "Untitled";

/// Errors from editor actions. Wraps the model's errors so the UI handles one type.
#[derive(Debug, Error)]
pub enum EditorError {
    /// The model rejected the command or could not read a page.
    #[error(transparent)]
    Model(#[from] ModelError),
}

/// One row of the sidebar's page tree, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarItem {
    /// The page this row opens.
    pub page: PageId,
    /// Text to show: the title, or [`UNTITLED`] when it is empty.
    pub label: String,
    /// The page's emoji, if any.
    pub icon: Option<String>,
    /// Whether this is the page currently open.
    pub selected: bool,
    /// How deeply the page is nested: 0 for a top-level page.
    pub depth: usize,
    /// Whether the page has pages inside it (outside the trash), so the row offers to expand.
    pub has_children: bool,
    /// Whether the page's children are listed below it.
    pub expanded: bool,
}

/// A workspace being edited in one window, plus that window's selected page.
pub struct WorkspaceEditor {
    workspace: Workspace,
    selected: Option<PageId>,
    /// The blocks of each page opened so far (loaded from disk, or new).
    pages: HashMap<PageId, PageEditor>,
    /// Pages whose children the sidebar lists. View state for this device, not part of the document.
    expanded: HashSet<PageId>,
    /// Pages opened in this window, most recent first, for the quick switcher.
    recent: Vec<PageId>,
}

impl WorkspaceEditor {
    /// Starts editing `workspace` with nothing selected.
    #[must_use]
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            selected: None,
            pages: HashMap::new(),
            expanded: HashSet::new(),
            recent: Vec::new(),
        }
    }

    /// The workspace being edited, for read queries.
    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Opens `page`. Fails if it is not in the workspace.
    pub fn select(&mut self, page: PageId) -> Result<(), EditorError> {
        if !self.workspace.contains(page) {
            return Err(ModelError::PageNotFound(page).into());
        }
        self.selected = Some(page);
        self.remember_opened(page);
        Ok(())
    }

    /// Pages opened in this window, most recent first (at most [`RECENT_LIMIT`]). Some may since have
    /// gone to the trash; the quick switcher leaves those out.
    #[must_use]
    pub fn recent_pages(&self) -> &[PageId] {
        &self.recent
    }

    /// Puts `page` at the front of the recently opened pages.
    pub(crate) fn remember_opened(&mut self, page: PageId) {
        self.recent.retain(|&seen| seen != page);
        self.recent.insert(0, page);
        self.recent.truncate(RECENT_LIMIT);
    }

    /// The open page's blocks, creating its document (with one empty paragraph to type in) the first
    /// time the page is opened. `None` when no page is open.
    pub fn open_page(&mut self) -> Result<Option<&mut PageEditor>, EditorError> {
        let Some(page) = self.selected else {
            return Ok(None);
        };
        let editor = match self.pages.entry(page) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let mut editor = PageEditor::new(PageDoc::new());
                editor.ensure_block()?;
                entry.insert(editor)
            }
        };
        Ok(Some(editor))
    }

    /// Hands over `page`'s blocks as loaded from disk, before the page is first opened. A page that
    /// is already in memory keeps its live copy (it may have newer edits). An empty page gets one
    /// paragraph to type in, as a new page does.
    pub fn insert_page(&mut self, page: PageId, doc: PageDoc) -> Result<(), EditorError> {
        if !self.workspace.contains(page) {
            return Err(ModelError::PageNotFound(page).into());
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = self.pages.entry(page) {
            let mut editor = PageEditor::new(doc);
            editor.ensure_block()?;
            entry.insert(editor);
        }
        Ok(())
    }

    /// Every page document in memory, for saving.
    pub fn loaded_pages(&self) -> impl Iterator<Item = (PageId, &PageDoc)> {
        self.pages.iter().map(|(page, editor)| (*page, editor.doc()))
    }

    /// A page's blocks, if it has been opened before; read-only.
    #[must_use]
    pub fn page_editor(&self, page: PageId) -> Option<&PageEditor> {
        self.pages.get(&page)
    }

    /// The page currently open, if any.
    #[must_use]
    pub fn selected(&self) -> Option<PageId> {
        self.selected
    }

    /// The open page's title exactly as stored (empty for a new page), for the title field.
    pub fn selected_title(&self) -> Result<Option<String>, EditorError> {
        match self.selected {
            Some(page) => Ok(Some(self.workspace.page(page)?.title)),
            None => Ok(None),
        }
    }

    /// Renames the open page. Does nothing when no page is open; see [`WorkspaceEditor::rename`].
    pub fn rename_selected(&mut self, title: &str) -> Result<(), EditorError> {
        match self.selected {
            Some(page) => self.rename(page, title),
            None => Ok(()),
        }
    }

    /// Sets `page`'s emoji icon, or removes it with `None`. Does nothing when it is unchanged, so picking
    /// the same emoji again records no change (or undo step).
    pub fn set_icon(&mut self, page: PageId, icon: Option<&str>) -> Result<(), EditorError> {
        if self.workspace.page(page)?.icon.as_deref() == icon {
            return Ok(());
        }
        self.workspace.apply(WorkspaceCommand::SetPageIcon {
            page,
            icon: icon.map(str::to_owned),
        })?;
        Ok(())
    }

    /// The open page's icon, if a page is open and has one.
    pub fn selected_icon(&self) -> Result<Option<String>, EditorError> {
        match self.selected {
            Some(page) => Ok(self.workspace.page(page)?.icon),
            None => Ok(None),
        }
    }

    /// Renames `page`. Does nothing when the title is unchanged, so refreshing a title field never
    /// records a pointless change (or undo step).
    pub fn rename(&mut self, page: PageId, title: &str) -> Result<(), EditorError> {
        if self.workspace.page(page)?.title == title {
            return Ok(());
        }
        self.workspace.apply(WorkspaceCommand::RenamePage {
            page,
            title: title.to_owned(),
        })?;
        Ok(())
    }
}

mod trash;
mod tree;

pub use trash::{TrashItem, TrashList, trashed_ago};
pub use tree::DropPlace;

#[cfg(test)]
mod tests;
