//! What the benches start from: a big page saved in a workspace folder, and an editor ready to type
//! in the middle of a big page.

use leaf_core::{BlockId, PageId};
use leaf_editor::{Caret, PageEditor, PageView};
use leaf_fixtures::big_page;
use leaf_model::{PageCommand, PageDoc};
use leaf_store::{DocKey, Workspace};

use crate::{BenchError, SAVED_AT, Scratch};

/// A workspace folder holding one big saved page: what "open a page" and "save" are measured on.
pub struct SavedPage {
    /// The open workspace folder.
    pub folder: Workspace,
    /// The saved page.
    pub page: PageId,
    // Declared last so the folder's database is closed before the folder is removed.
    _scratch: Scratch,
}

impl SavedPage {
    /// A new scratch workspace folder with a `blocks`-block page saved in it, outline fixed by `seed`.
    pub fn new(seed: u64, blocks: usize) -> Result<Self, BenchError> {
        let scratch = Scratch::new();
        let mut folder = Workspace::create(scratch.path())?;
        let page = PageId::new();
        let doc = big_page(seed, blocks)?;
        folder.store_mut().save(DocKey::Page(page), &doc, SAVED_AT)?;
        Ok(Self {
            folder,
            page,
            _scratch: scratch,
        })
    }

    /// Opens the page as the app does: loads it from the store, starts an editor on it and builds
    /// the view model the window draws.
    pub fn open(&self) -> Result<PageEditor, BenchError> {
        let doc: PageDoc = self
            .folder
            .store()
            .load(DocKey::Page(self.page))?
            .ok_or(BenchError::Setup("the page was not saved"))?;
        let mut editor = PageEditor::new(doc);
        editor.view()?;
        Ok(editor)
    }
}

/// An editor on a `blocks`-block page (outline fixed by `seed`), its view model built and the caret
/// placed by [`caret_mid_page`]: where typing is measured.
pub fn typing_editor(seed: u64, blocks: usize) -> Result<PageEditor, BenchError> {
    let mut editor = PageEditor::new(big_page(seed, blocks)?);
    caret_mid_page(&mut editor)?;
    Ok(editor)
}

/// Puts the caret at the start of the first text block from the middle of the page on, so typing
/// is measured away from the page's ends, where a slow path could hide.
pub fn caret_mid_page(editor: &mut PageEditor) -> Result<(), BenchError> {
    let rows = editor.view()?;
    let block = rows
        .iter()
        .skip(rows.len() / 2)
        .find(|row| row.text.is_some())
        .map(|row| row.id)
        .ok_or(BenchError::Setup("no text block in the second half of the page"))?;
    editor.set_caret(Caret { block, at: 0 })?;
    Ok(())
}

/// A big page and its view model kept apart from an editor, so updating the view after a change can
/// be timed on its own. (A `PageEditor` updates its view inside each keystroke, so there the update
/// is part of the keystroke's time.)
pub struct ViewedPage {
    doc: PageDoc,
    view: PageView,
    block: BlockId,
}

impl ViewedPage {
    /// A `blocks`-block page (outline fixed by `seed`) with its view model built, typing into the
    /// first text block from the middle of the page on.
    pub fn new(seed: u64, blocks: usize) -> Result<Self, BenchError> {
        let doc = big_page(seed, blocks)?;
        let mut view = PageView::default();
        view.refresh(&doc)?;
        let block = view
            .rows()
            .iter()
            .skip(view.rows().len() / 2)
            .find(|row| row.text.is_some())
            .map(|row| row.id)
            .ok_or(BenchError::Setup("no text block in the second half of the page"))?;
        Ok(Self { doc, view, block })
    }

    /// Types one character at the start of the block, through the model command, leaving the view
    /// out of date.
    pub fn type_char(&mut self) -> Result<(), BenchError> {
        self.doc.apply(PageCommand::InsertText {
            id: self.block,
            pos: 0,
            text: "a".to_owned(),
        })?;
        Ok(())
    }

    /// Brings the view model up to date with the page, returning how many rows it has.
    pub fn refresh(&mut self) -> Result<usize, BenchError> {
        self.view.refresh(&self.doc)?;
        Ok(self.view.rows().len())
    }

    /// The text of the block being typed in, as the view model shows it.
    #[must_use]
    pub fn shown_text(&self) -> Option<&str> {
        let row = self.view.rows().iter().find(|row| row.id == self.block)?;
        row.text.as_deref()
    }
}

#[cfg(test)]
mod tests;
