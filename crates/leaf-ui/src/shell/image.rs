//! Inserting images (E05-S07, ADR-051): the file picker that the slash menu's "Image" asks for, and
//! pictures pasted from other apps. Either way the picture is stored in the workspace's asset store
//! first, then `PageEditor::insert_image` decides where the block goes.

use std::path::{Path, PathBuf};

use gpui::{Context, PathPromptOptions, Window};
use leaf_editor::PageEditor;

use super::{Shell, report};

/// Where a picture to insert comes from.
pub(super) enum ImageSource<'a> {
    /// An image file the owner picked or copied in the file manager.
    File(&'a Path),
    /// Image bytes from the clipboard (a screenshot, "Copy image" in a browser).
    Bytes(&'a [u8]),
}

impl Shell {
    /// Shows the image file picker if the page just asked for one (the slash menu's "Image"). The
    /// chosen file is inserted where `/image` was typed; closing the picker inserts nothing.
    pub(super) fn pick_image_if_asked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.page().is_some_and(PageEditor::take_image_request) {
            return;
        }
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Insert image".into()),
        });
        cx.spawn_in(window, async move |shell, cx| {
            let path: Option<PathBuf> = match chosen.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Err(err)) => {
                    report("could not show the file picker", err);
                    None
                }
                Ok(Ok(None)) | Err(_) => None,
            };
            let _ = shell.update_in(cx, |shell, window, cx| match path {
                Some(path) => {
                    shell.insert_image(&ImageSource::File(&path), window, cx);
                }
                None => {
                    if let Some(page) = shell.page() {
                        page.cancel_image();
                    }
                }
            });
        })
        .detach();
    }

    /// Stores the picture in the workspace and adds an image block for it. Returns whether it was
    /// inserted: `false` when the file isn't an image or the window has no workspace folder.
    pub(super) fn insert_image(
        &mut self,
        source: &ImageSource<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(storage) = self.storage.as_mut() else {
            report(
                "could not insert the image",
                "this window has no workspace folder",
            );
            return false;
        };
        let stored = match source {
            ImageSource::File(path) => storage.import_image_file(path),
            ImageSource::Bytes(bytes) => storage.import_image(bytes),
        };
        let inserted = match stored {
            Ok(asset) => match self.page().map(|page| page.insert_image(asset)) {
                Some(Ok(Some(_))) => true,
                Some(Err(err)) => {
                    report("could not insert the image", err);
                    false
                }
                Some(Ok(None)) | None => false,
            },
            Err(err) => {
                report("could not store the image", err);
                if let Some(page) = self.page() {
                    page.cancel_image();
                }
                false
            }
        };
        self.changed(window, cx);
        inserted
    }
}

#[cfg(test)]
mod tests;
