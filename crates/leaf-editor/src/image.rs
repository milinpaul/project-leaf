//! Image blocks: where a new picture goes, and what a paste from another app brings in (E05-S07).
//!
//! The editor never touches files. The UI shows the file picker or reads the clipboard, stores the
//! picture in the workspace's asset store and hands over its [`AssetId`]; the editor decides where
//! the block goes. A picture asked for from the slash menu goes where the `/image` was typed, even if
//! the caret moved while the picker was open, because that place is kept as an anchor.

use leaf_core::AssetId;
use leaf_model::{Anchor, BlockKind, Caret, PageCommand};

use crate::page_editor::insert_after;
use crate::{EditorError, PageEditor};

/// A picture asked for (from the slash menu) and not yet given.
pub(crate) struct ImageRequest {
    /// Where the `/image` was typed.
    at: Anchor,
    /// Whether the UI has already been told to show the picker.
    asked: bool,
}

/// What a paste from outside Leaf should insert, given what the clipboard holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKind {
    /// Leaf's own blocks, copied from a Leaf page.
    Fragment,
    /// Plain text or Markdown.
    Text,
    /// A picture (a screenshot, "Copy image" in a browser, or an image file copied in the file manager).
    Image,
    /// Nothing Leaf can use.
    Nothing,
}

/// Chooses what to paste. Leaf's own blocks win, then text, then a picture: spreadsheets and word
/// processors put a picture of the copied cells next to their text, and the owner wants the text.
/// A picture pastes when it is all there is, as with a screenshot or a copied image.
#[must_use]
pub fn paste_kind(has_fragment: bool, text: Option<&str>, has_image: bool) -> PasteKind {
    if has_fragment {
        PasteKind::Fragment
    } else if text.is_some_and(|text| !text.is_empty()) {
        PasteKind::Text
    } else if has_image {
        PasteKind::Image
    } else {
        PasteKind::Nothing
    }
}

impl PageEditor {
    /// Remembers that a picture was asked for at `caret`, so the UI shows the file picker.
    pub(crate) fn request_image(&mut self, caret: Caret) -> Result<(), EditorError> {
        self.image = Some(ImageRequest {
            at: self.doc.anchor(caret)?,
            asked: false,
        });
        Ok(())
    }

    /// Whether the UI should show the image file picker now. True once per request (choosing
    /// "Image" in the slash menu), so the picker never opens twice for one choice.
    pub fn take_image_request(&mut self) -> bool {
        match &mut self.image {
            Some(request) if !request.asked => {
                request.asked = true;
                true
            }
            _ => false,
        }
    }

    /// The picker was closed without a file: forget the request.
    pub fn cancel_image(&mut self) {
        self.image = None;
    }

    /// Adds an image block showing `asset` (already in the asset store), with an empty caption, and
    /// puts the caret in the caption. It goes where the picture was asked for, else at the caret: an
    /// empty text block becomes the image, otherwise the image goes below it. One undo step. Returns
    /// the caret, or `None` when there is nowhere to put it (no caret and no request).
    pub fn insert_image(&mut self, asset: AssetId) -> Result<Option<Caret>, EditorError> {
        let requested = match self.image.take() {
            Some(request) => self.doc.resolve(&request.at)?,
            None => None,
        };
        let Some(target) = requested.or(self.caret()?) else {
            return Ok(None);
        };
        self.begin()?;
        let block = target.block;
        let caret = self.doc.as_separate_step(|doc| -> Result<Caret, EditorError> {
            let image = BlockKind::Image {
                asset,
                width: None,
                alt: String::new(),
            };
            let kind = doc.kind(block)?;
            let empty = !matches!(kind, BlockKind::Image { .. })
                && doc.plain_text(block)?.is_some_and(|text| text.is_empty());
            if empty {
                doc.apply(PageCommand::SetBlockKind {
                    id: block,
                    kind: image,
                })?;
                return Ok(Caret { block, at: 0 });
            }
            Ok(Caret {
                block: insert_after(doc, block, image)?,
                at: 0,
            })
        })?;
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }
}

#[cfg(test)]
mod tests;
