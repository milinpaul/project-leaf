//! Editing the workspace: which page is selected, what the sidebar shows, and the page actions the
//! sidebar offers. Turns those actions into `WorkspaceCommand`s; never touches Loro itself.

use std::collections::HashMap;

use leaf_core::PageId;
use leaf_model::{ModelError, Outcome, PageDoc, Workspace, WorkspaceCommand};

use crate::PageEditor;
use thiserror::Error;

/// What the sidebar shows for an empty title, so a new page is never an invisible row.
pub const UNTITLED: &str = "Untitled";

/// Errors from editor actions. Wraps the model's errors so the UI handles one type.
#[derive(Debug, Error)]
pub enum EditorError {
    /// The model rejected the command or could not read a page.
    #[error(transparent)]
    Model(#[from] ModelError),
}

/// One row of the sidebar, ready to draw.
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
}

/// A workspace being edited in one window, plus that window's selected page.
pub struct WorkspaceEditor {
    workspace: Workspace,
    selected: Option<PageId>,
    /// Each opened page's blocks, kept in memory until saving arrives (E03).
    pages: HashMap<PageId, PageEditor>,
}

impl WorkspaceEditor {
    /// Starts editing `workspace` with nothing selected.
    #[must_use]
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            selected: None,
            pages: HashMap::new(),
        }
    }

    /// The workspace being edited, for read queries.
    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Top-level pages in order, as sidebar rows. Pages in the trash are hidden by the model.
    ///
    /// Nested pages arrive with the full page tree (E06-S03).
    pub fn sidebar(&self) -> Result<Vec<SidebarItem>, EditorError> {
        let mut items = Vec::new();
        for page in self.workspace.root_pages()? {
            let meta = self.workspace.page(page)?;
            let label = if meta.title.is_empty() {
                UNTITLED.to_owned()
            } else {
                meta.title
            };
            items.push(SidebarItem {
                page,
                label,
                icon: meta.icon,
                selected: self.selected == Some(page),
            });
        }
        Ok(items)
    }

    /// Adds an untitled page at the end of the top level and selects it, so the user can type its
    /// title straight away. `now_ms` is the current time in Unix milliseconds, from the app.
    pub fn new_page(&mut self, now_ms: i64) -> Result<PageId, EditorError> {
        let index = self.workspace.root_pages()?.len();
        let command = WorkspaceCommand::CreatePage {
            parent: None,
            index,
            title: String::new(),
            created_at: now_ms,
        };
        let Outcome::PageCreated(page) = self.workspace.apply(command)? else {
            unreachable!("CreatePage always reports the page it created");
        };
        self.selected = Some(page);
        Ok(page)
    }

    /// Opens `page`. Fails if it is not in the workspace.
    pub fn select(&mut self, page: PageId) -> Result<(), EditorError> {
        if !self.workspace.contains(page) {
            return Err(ModelError::PageNotFound(page).into());
        }
        self.selected = Some(page);
        Ok(())
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

    /// Renames the open page. Does nothing when no page is open or the title is unchanged, so
    /// refreshing the title field never records a pointless change (or undo step).
    pub fn rename_selected(&mut self, title: &str) -> Result<(), EditorError> {
        let Some(page) = self.selected else { return Ok(()) };
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

#[cfg(test)]
mod tests;
