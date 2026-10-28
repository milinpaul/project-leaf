//! The shell's link to a workspace folder on disk: loading pages when first opened, and running the
//! autosave timer.
//!
//! All saving decisions live in `leaf-store`'s `Autosave` (tested there with a fake clock). This
//! only supplies the clock, the timer and the list of open documents.

use std::time::Duration;

use gpui::{Context, Window};
use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace};
use leaf_store::{Autosave, DocKey, Savable, StoreError, Workspace as Folder};

use super::{Shell, now_ms, report};

/// How often the autosave timer checks for due saves. Small next to the 300 ms delay, so a change
/// is on disk well within the spec's one second.
const TICK: Duration = Duration::from_millis(100);

/// An open workspace folder and its autosave state: what a window needs to save to disk.
pub struct Storage {
    folder: Folder,
    autosave: Autosave,
}

impl Storage {
    /// Opens `folder`'s workspace document, or starts an empty one for a new folder.
    pub fn open(folder: Folder) -> Result<(Self, Workspace), StoreError> {
        let workspace = folder
            .store()
            .load::<Workspace>(DocKey::Workspace)?
            .unwrap_or_default();
        let storage = Self {
            folder,
            autosave: Autosave::default(),
        };
        Ok((storage, workspace))
    }

    /// The saved blocks of `page`, or `None` for a page never saved (a new one).
    fn load_page(&self, page: PageId) -> Result<Option<PageDoc>, StoreError> {
        self.folder.store().load(DocKey::Page(page))
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
            .push(cx.observe_window_activation(window, |shell, window, _| {
                if !window.is_window_active() {
                    shell.save(true);
                }
            }));
        self.subscriptions.push(cx.on_app_quit(|shell, _| {
            shell.save(true);
            async {}
        }));
    }

    /// Saves what is due (`flush` false, from the timer) or everything unsaved (`flush` true).
    pub(super) fn save(&mut self, flush: bool) {
        let Some(storage) = &mut self.storage else {
            return;
        };
        let mut docs: Vec<(DocKey, &dyn Savable)> = vec![(DocKey::Workspace, self.editor.workspace())];
        docs.extend(
            self.editor
                .loaded_pages()
                .map(|(page, doc)| (DocKey::Page(page), doc as &dyn Savable)),
        );
        let store = storage.folder.store_mut();
        let result = if flush {
            storage.autosave.flush(store, &docs, now_ms())
        } else {
            storage.autosave.tick(store, &docs, now_ms())
        };
        if let Err(err) = result {
            report("could not save", err);
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
