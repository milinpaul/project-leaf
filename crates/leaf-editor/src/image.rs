//! Image blocks: where a new picture goes, what a paste from another app brings in (E05-S07), and how
//! big a picture is drawn and resized (E07-S08).
//!
//! The editor never touches files. The UI shows the file picker or reads the clipboard, stores the
//! picture in the workspace's asset store and hands over its [`AssetId`]; the editor decides where
//! the block goes. A picture asked for from the slash menu goes where the `/image` was typed, even if
//! the caret moved while the picker was open, because that place is kept as an anchor.

use leaf_core::{AssetId, BlockId};
use leaf_model::{Anchor, BlockKind, BlockProp, Caret, PageCommand};

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

/// The narrowest an image can be resized to, in logical pixels: still recognisable, still grabbable.
pub const MIN_IMAGE_WIDTH: f32 = 64.0;

/// How big to draw a picture whose natural size is `natural` (width, height, in pixels): the width the
/// owner resized it to, else its natural width, never wider than `available` (the page's column at
/// the image's indent), and the height that keeps its shape. A picture with no size draws nothing.
#[must_use]
pub fn image_display_size(stored: Option<f64>, natural: (f32, f32), available: f32) -> (f32, f32) {
    let (natural_width, natural_height) = natural;
    if natural_width <= 0.0 || natural_height <= 0.0 {
        return (0.0, 0.0);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "widths are a few thousand pixels at most"
    )]
    let wanted = stored.map_or(natural_width, |width| width as f32);
    let width = wanted.min(available).max(1.0);
    (width, width * natural_height / natural_width)
}

/// The width while dragging an image's resize handle: the width when the drag began plus how far the
/// pointer moved, kept between [`MIN_IMAGE_WIDTH`] and `available`, in whole pixels.
#[must_use]
pub fn resized_image_width(start: f32, moved: f32, available: f32) -> f32 {
    let widest = available.max(1.0);
    (start + moved).clamp(MIN_IMAGE_WIDTH.min(widest), widest).round()
}

impl PageEditor {
    /// Sets an image's width (`None` for its natural size), as one undo step. Does nothing for other
    /// blocks or when the width is unchanged, so a click on the handle without a drag records nothing.
    pub fn set_image_width(&mut self, id: BlockId, width: Option<f64>) -> Result<(), EditorError> {
        let BlockKind::Image { width: current, .. } = self.doc.kind(id)? else {
            return Ok(());
        };
        if current == width {
            return Ok(());
        }
        self.begin()?;
        self.doc.as_separate_step(|doc| {
            doc.apply(PageCommand::SetProp {
                id,
                prop: BlockProp::ImageWidth(width),
            })
        })?;
        self.finish()?;
        Ok(())
    }

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
        // A picture asked for in a table cell goes below the table.
        let block = crate::table::table_of(&self.doc, target.block)?.unwrap_or(target.block);
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
