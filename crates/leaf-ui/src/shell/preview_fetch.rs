//! Fetching web bookmark previews (E15-S05, ADR-080): the page editor notes which bookmarks want one,
//! and here each is fetched with `leaf-fetch` on a background thread, so typing never waits. The
//! pictures go into the workspace's asset store and the preview into the bookmark, off the undo
//! history. A fetch that fails changes nothing: the card keeps showing the address.
//!
//! Nothing is fetched unless the owner allowed previews in the app settings (off by default). The first
//! time a bookmark wants a preview before they have answered, a bar at the bottom of the page asks once
//! (E15-S06, `previews_question`); "Not now" is never asked again, and Settings › Privacy changes it.

use std::sync::Arc;

use gpui::Context;
use leaf_core::BlockId;
use leaf_fetch::{Fetcher, Picture, Preview};
use leaf_model::WebPreview;

use super::{Shell, now_ms, report};

impl Shell {
    /// Starts a fetch for each bookmark the page asked a preview for, if the owner allows previews; asks
    /// them first if they never answered. Called after every page action.
    pub(super) fn fetch_previews_if_asked(&mut self, cx: &mut Context<Self>) {
        let settings = crate::settings::get(cx);
        let Some(page) = self.page() else { return };
        if !settings.web_previews_answered() {
            // The requests wait for the answer.
            if page.has_preview_requests() && !self.blocks.asking_previews {
                self.blocks.asking_previews = true;
                cx.notify();
            }
            return;
        }
        page.set_previews_allowed(settings.web_previews_allowed());
        for (block, url) in page.take_preview_requests() {
            self.fetch_preview(block, url, cx);
        }
    }

    /// Fetches a preview for every bookmark on the open page that never had one, if previews are allowed:
    /// when a page opens and when previews are turned on.
    pub(super) fn fetch_missing_previews(&mut self, cx: &mut Context<Self>) {
        if !crate::settings::get(cx).web_previews_allowed() {
            return;
        }
        if let Some(Err(err)) = self.page().map(leaf_editor::PageEditor::request_missing_previews) {
            report("could not look for web bookmarks without a preview", err);
        }
        self.fetch_previews_if_asked(cx);
    }

    /// Fetches `url`'s preview for `block` off the UI thread; the card says so meanwhile.
    fn fetch_preview(&mut self, block: BlockId, url: String, cx: &mut Context<Self>) {
        if !self.blocks.fetching.insert(block) {
            return; // already on its way
        }
        let fetcher = Arc::clone(
            self.blocks
                .fetcher
                .get_or_insert_with(|| Arc::new(Fetcher::new())),
        );
        let background = cx.background_executor().clone();
        cx.spawn(async move |shell, cx| {
            let address = url.clone();
            let outcome = background.spawn(async move { fetcher.preview(&address) }).await;
            let _ = shell.update(cx, |shell, cx| {
                shell.blocks.fetching.remove(&block);
                match outcome {
                    Ok(preview) => shell.preview_arrived(block, &url, preview),
                    Err(err) => report("could not fetch the web bookmark's preview", err),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Stores the preview's pictures and saves it into the bookmark, if it is still one of `url`.
    fn preview_arrived(&mut self, block: BlockId, url: &str, found: Preview) {
        let mut store = |picture: Option<Picture>| {
            let storage = self.storage.as_mut()?;
            picture.and_then(|picture| storage.import_image(&picture.bytes).ok())
        };
        let preview = WebPreview {
            title: found.title,
            description: found.description,
            image: store(found.image),
            favicon: store(found.favicon),
            fetched_at: Some(now_ms()),
        };
        if let Some(Err(err)) = self
            .page()
            .map(|page| page.set_web_bookmark_preview(block, url, preview))
        {
            report("could not save the web bookmark's preview", err);
        }
    }
}

#[cfg(test)]
mod tests;
