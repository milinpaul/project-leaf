//! Which web bookmarks want their preview fetched (E15-S05, ADR-080). The editor never goes online: it
//! notes a request when the owner makes a bookmark on this device (pasting, `/web bookmark`, Turn into)
//! or asks with "Refresh preview", and the app takes the requests, fetches with `leaf-fetch` off the UI
//! thread and hands the result back to [`PageEditor::set_web_bookmark_preview`]. While previews aren't
//! allowed nothing is handed out, and what waited is dropped, so turning them on later never fetches for
//! bookmarks made before.

use leaf_core::BlockId;
use leaf_model::{PageCommand, PageOutcome, WebPreview};

use crate::{EditorError, PageEditor};

/// The requests waiting, and whether the owner allows previews at all.
#[derive(Debug, Default)]
pub(crate) struct PreviewRequests {
    /// Whether the owner allowed fetching previews (the app's setting; off until they say yes).
    pub(crate) allowed: bool,
    /// Bookmarks and the address each was made with, oldest first, without repeats.
    waiting: Vec<(BlockId, String)>,
}

impl PageEditor {
    /// Tells the editor whether the owner allows fetching previews. While they don't, no request is
    /// noted, "Refresh preview" isn't offered, and requests already waiting are dropped.
    pub fn set_previews_allowed(&mut self, allowed: bool) {
        self.previews.allowed = allowed;
        if !allowed {
            self.previews.waiting.clear();
        }
    }

    /// The bookmarks to fetch a preview for, with their addresses, and empties the list; nothing while
    /// previews aren't allowed. The app tells the editor whether they are just before taking.
    pub fn take_preview_requests(&mut self) -> Vec<(BlockId, String)> {
        let waiting = std::mem::take(&mut self.previews.waiting);
        if self.previews.allowed {
            waiting
        } else {
            Vec::new()
        }
    }

    /// Notes that `block`, a bookmark of `url`, wants its preview.
    pub(crate) fn want_preview(&mut self, block: BlockId, url: &str) {
        let waiting = &mut self.previews.waiting;
        if !waiting.iter().any(|(b, _)| *b == block) {
            waiting.push((block, url.to_owned()));
        }
    }

    /// Saves a fetched preview into `block`, if it is still a bookmark of `url` (the owner may have
    /// changed or deleted it while the fetch ran). It is never an undo step. Returns whether it changed.
    pub fn set_web_bookmark_preview(
        &mut self,
        block: BlockId,
        url: &str,
        preview: WebPreview,
    ) -> Result<bool, EditorError> {
        let outcome = self.doc.apply(PageCommand::SetWebBookmarkPreview {
            id: block,
            url: url.to_owned(),
            preview,
        })?;
        Ok(outcome != PageOutcome::Unchanged)
    }
}

#[cfg(test)]
mod tests;
