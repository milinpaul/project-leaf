//! Fetching web bookmark previews (E15-S05, ADR-080): the page editor notes which bookmarks want one,
//! and here each is fetched with `leaf-fetch` on a background thread, so typing never waits. The
//! pictures go into the workspace's asset store and the preview into the bookmark, off the undo
//! history. A fetch that fails changes nothing: the card keeps showing the address.
//!
//! Nothing is fetched unless the owner allowed previews in the app settings (off by default); the
//! editor isn't even told to note requests until then.

use std::sync::Arc;

use gpui::{Context, Window};
use leaf_core::BlockId;
use leaf_fetch::{Fetcher, Picture, Preview};
use leaf_model::WebPreview;

use super::{Shell, now_ms, report};

impl Shell {
    /// Starts a fetch for each bookmark the page asked a preview for, if the owner allows previews.
    /// Called after every page action.
    pub(super) fn fetch_previews_if_asked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let allowed = crate::settings::get(cx).web_previews_allowed();
        let Some(page) = self.page() else { return };
        page.set_previews_allowed(allowed);
        for (block, url) in page.take_preview_requests() {
            self.fetch_preview(block, url, window, cx);
        }
    }

    /// Fetches `url`'s preview for `block` off the UI thread; the card says so meanwhile.
    fn fetch_preview(&mut self, block: BlockId, url: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.blocks.fetching.insert(block) {
            return; // already on its way
        }
        let fetcher = Arc::clone(
            self.blocks
                .fetcher
                .get_or_insert_with(|| Arc::new(Fetcher::new())),
        );
        let background = cx.background_executor().clone();
        cx.spawn_in(window, async move |shell, cx| {
            let address = url.clone();
            let outcome = background.spawn(async move { fetcher.preview(&address) }).await;
            let _ = shell.update_in(cx, |shell, _, cx| {
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
