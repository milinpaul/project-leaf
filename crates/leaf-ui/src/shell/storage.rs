//! The shell's link to a workspace folder on disk: loading pages when first opened, running the
//! autosave timer, and telling the search indexer when something was saved.
//!
//! All saving decisions live in `leaf-store`'s `Autosave` (tested there with a fake clock). This
//! only supplies the clock, the timer and the list of open documents.

use std::time::Duration;

use gpui::{Context, Window};
use leaf_core::{AssetId, DatabaseId, PageId};
use leaf_model::{Database, PageDoc, Workspace};
use leaf_search::SearchWorker;
use leaf_store::{Autosave, DocKey, Savable, StoreError, ViewState, Workspace as Folder};

use super::{Shell, now_ms, report};

mod search_source;

/// How often the autosave timer checks for due saves. Small next to the 300 ms delay, so a change
/// is on disk well within the spec's one second.
const TICK: Duration = Duration::from_millis(100);

/// An open workspace folder and its autosave state: what a window needs to save to disk.
pub struct Storage {
    folder: Folder,
    autosave: Autosave,
    /// Keeps the search index current on its own thread; `None` if it couldn't start (search is
    /// then unavailable, editing is not affected).
    search: Option<SearchWorker>,
    /// How the sidebar was left (expanded pages), as last saved to `view.json`.
    view: ViewState,
}

impl Storage {
    /// Opens `folder`'s workspace document, or starts an empty one for a new folder.
    pub fn open(folder: Folder) -> Result<(Self, Workspace), StoreError> {
        let workspace = folder
            .store()
            .load::<Workspace>(DocKey::Workspace)?
            .unwrap_or_default();
        let search = match folder.reader() {
            Ok(reader) => Some(SearchWorker::start(
                folder.root().to_owned(),
                search_source::StoreSource(reader),
            )),
            Err(err) => {
                report("could not start search", err);
                None
            }
        };
        let view = ViewState::load(&folder.view_state_path());
        let storage = Self {
            folder,
            autosave: Autosave::default(),
            search,
            view,
        };
        Ok((storage, workspace))
    }

    /// The workspace folder.
    pub fn root(&self) -> &std::path::Path {
        self.folder.root()
    }

    /// The pages that were expanded in the sidebar last time.
    pub(super) fn expanded(&self) -> Vec<PageId> {
        self.view.expanded()
    }

    /// Saves the expanded pages to `view.json`, if they changed.
    pub(super) fn save_expanded(&mut self, pages: &[PageId]) -> Result<(), StoreError> {
        if self.view.expanded() == pages {
            return Ok(());
        }
        self.view.set_expanded(pages);
        self.view.save(&self.folder.view_state_path())
    }

    /// The page that was open when this workspace was last closed on this computer (E06-S19).
    pub(super) fn open_page(&self) -> Option<PageId> {
        self.view.open_page()
    }

    /// Saves the open page to `view.json`, if it changed, so it opens again next time.
    pub(super) fn save_open_page(&mut self, page: Option<PageId>) -> Result<(), StoreError> {
        if self.view.open_page() == page {
            return Ok(());
        }
        self.view.set_open_page(page);
        self.view.save(&self.folder.view_state_path())
    }

    /// Stores the picture `bytes` in the workspace's `assets/` folder (ADR 0047) and returns its id.
    /// Refused unless the bytes are an image.
    pub(super) fn import_image(&mut self, bytes: &[u8]) -> Result<AssetId, StoreError> {
        self.folder.import_asset(bytes, now_ms()).map(|asset| asset.id)
    }

    /// Where the picture `id` is kept, if it is safe to draw (an SVG is checked first, ADR 0076).
    pub(super) fn drawable_asset(&self, id: AssetId) -> Result<std::path::PathBuf, leaf_store::SvgProblem> {
        self.folder.drawable_asset(id)
    }

    /// Copies the image file at `path` into the workspace's `assets/` folder and returns its id.
    pub(super) fn import_image_file(&mut self, path: &std::path::Path) -> Result<AssetId, StoreError> {
        self.folder
            .import_asset_file(path, now_ms())
            .map(|asset| asset.id)
    }

    /// The saved blocks of `page`, or `None` for a page never saved (a new one).
    fn load_page(&self, page: PageId) -> Result<Option<PageDoc>, StoreError> {
        self.folder.store().load(DocKey::Page(page))
    }

    /// The saved database `id`, or `None` for one never saved.
    pub(super) fn load_database(&self, id: DatabaseId) -> Result<Option<Database>, StoreError> {
        self.folder.store().load(DocKey::Database(id))
    }
}

impl Shell {
    /// Starts the autosave timer and saves everything when the window loses focus or the app quits.
    pub(super) fn start_autosave(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if shell.update(cx, |shell, _| shell.save(false)).is_err() {
                    break; // the window is gone
                }
            }
        })
        .detach();
        self.subscriptions
            .push(cx.observe_window_activation(window, |shell, window, cx| {
                if !window.is_window_active() {
                    shell.save(true);
                    crate::settings::save(cx);
                }
            }));
        self.subscriptions.push(cx.on_app_quit(|shell, _| {
            shell.save(true);
            async {}
        }));
    }

    /// Saves what is due (`flush` false, from the timer) or everything unsaved (`flush` true).
    pub(super) fn save(&mut self, flush: bool) {
        // First, so the edited times go out with the edits they describe.
        if let Err(err) = self.editor.stamp_edits(now_ms()) {
            report("could not record when pages were edited", err);
        }
        let Some(storage) = &mut self.storage else {
            return;
        };
        let mut docs: Vec<(DocKey, &dyn Savable)> = vec![(DocKey::Workspace, self.editor.workspace())];
        docs.extend(
            self.editor
                .loaded_pages()
                .map(|(page, doc)| (DocKey::Page(page), doc as &dyn Savable)),
        );
        docs.extend(
            self.editor
                .loaded_databases()
                .map(|(id, db)| (DocKey::Database(id), db as &dyn Savable)),
        );
        let store = storage.folder.store_mut();
        let result = if flush {
            storage.autosave.flush(store, &docs, now_ms())
        } else {
            storage.autosave.tick(store, &docs, now_ms())
        };
        match result {
            Ok(0) => {}
            Ok(_) => {
                if let Some(search) = &storage.search {
                    search.poke(); // never blocks: indexing runs on its own thread
                }
            }
            Err(err) => report("could not save", err),
        }
    }

    /// Removes permanently deleted pages from disk. The workspace (which no longer lists them) is saved
    /// first, so a crash in between leaves unused documents behind, never a live page without its
    /// blocks. Search forgets them on its next pass.
    pub(super) fn remove_from_disk(&mut self, pages: &[PageId]) {
        if pages.is_empty() {
            return;
        }
        self.save(true);
        let Some(storage) = &mut self.storage else {
            return;
        };
        for &page in pages {
            storage.autosave.forget(DocKey::Page(page));
            if let Err(err) = storage.folder.store_mut().delete(DocKey::Page(page)) {
                report("could not remove a deleted page from disk", err);
            }
        }
        if let Some(search) = &storage.search {
            search.poke();
        }
    }

    /// Loads the database a row page belongs to, if it isn't loaded yet, so restoring the page from the Trash can
    /// bring its row back (E09-S03). Does nothing for other pages.
    pub(super) fn load_row_database(&mut self, page: PageId) {
        let Some(id) = self
            .editor
            .workspace()
            .page(page)
            .ok()
            .and_then(|meta| meta.row_of)
        else {
            return;
        };
        if self.editor.database(id).is_some() {
            return;
        }
        let Some(storage) = &self.storage else {
            return;
        };
        match storage.load_database(id) {
            Ok(database) => self.editor.insert_database(id, database.unwrap_or_default()),
            Err(err) => report("could not load the database", err),
        }
    }

    /// Brings the selected page's saved blocks into the editor the first time it is opened.
    pub(super) fn load_selected_page(&mut self) {
        let (Some(storage), Some(page)) = (&self.storage, self.editor.selected()) else {
            return;
        };
        if self.editor.page_editor(page).is_some() {
            return;
        }
        match storage.load_page(page) {
            Ok(Some(doc)) => {
                if let Err(err) = self.editor.insert_page(page, doc) {
                    report("could not open the saved page", err);
                }
            }
            Ok(None) => {} // never saved: the editor starts it empty
            Err(err) => report("could not load the page", err),
        }
    }
}

#[cfg(test)]
mod tests;
