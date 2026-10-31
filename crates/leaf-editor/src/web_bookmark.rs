//! Making web bookmarks in the editor (E15-S03, ADR-087): the offer after pasting a lone web
//! address, the address asked for by the slash menu's "Web bookmark", and where a new bookmark goes.
//!
//! A pasted address goes in as a link first (E17-S10), so nothing changes unless the owner asks. The
//! offer under it ("Dismiss", "Create web bookmark") stays only while the caret sits right after the
//! untouched link: typing, moving or clicking elsewhere ends it, and so does any key the offer doesn't
//! use. Choosing the bookmark is one undo step of its own, so undo gives the link back.

use leaf_core::BlockId;
use leaf_model::{Anchor, BlockKind, Caret, PageCommand, PageDoc, WebPreview, check_web_address};

use crate::format::char_marks;
use crate::page_editor::insert_after;
use crate::{EditorError, PageEditor, Vertical, openable_url};

/// The offer's entries, in order. The first, keeping the link, is highlighted to start with.
pub const BOOKMARK_OFFER_CHOICES: [&str; 2] = ["Dismiss", "Create web bookmark"];

/// The offer as the editor keeps it.
pub(crate) struct OfferState {
    /// Where the pasted link starts.
    start: Anchor,
    /// Its length in characters.
    len: usize,
    /// Its address.
    url: String,
    /// The highlighted entry.
    selected: usize,
}

/// The offer as the UI draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkOffer {
    /// Right after the pasted link, where the caret is: the menu goes under it.
    pub at: Caret,
    /// The pasted link's length in characters: it ends at `at`.
    pub len: usize,
    /// The address a bookmark would hold.
    pub url: String,
    /// The highlighted entry of [`BOOKMARK_OFFER_CHOICES`].
    pub selected: usize,
}

/// A web address asked for from the slash menu, until it is given or the prompt is cancelled.
pub(crate) struct AddressRequest {
    /// Where `/web bookmark` was typed.
    at: Anchor,
    /// Whether the UI has already been told to show the prompt.
    asked: bool,
}

impl PageEditor {
    /// Offers a bookmark for the link just pasted at `start..start + len`, if `url` is a web address a
    /// bookmark can hold (not `mailto:`).
    pub(crate) fn offer_bookmark(&mut self, start: Caret, len: usize, url: &str) -> Result<(), EditorError> {
        self.offer = None;
        if len > 0 && check_web_address(url).is_ok() {
            self.offer = Some(OfferState {
                start: self.doc.anchor(start)?,
                len,
                url: url.to_owned(),
                selected: 0,
            });
        }
        Ok(())
    }

    /// The offer, if it still stands: the caret is right after the pasted link, which is still there
    /// and still links to the address. Otherwise it is forgotten.
    pub fn bookmark_offer(&mut self) -> Result<Option<BookmarkOffer>, EditorError> {
        let Some(offer) = &self.offer else {
            return Ok(None);
        };
        let (anchor, len, url, selected) =
            (offer.start.clone(), offer.len, offer.url.clone(), offer.selected);
        let start = self.doc.resolve(&anchor)?;
        let at = start.map(|start| Caret {
            at: start.at + len,
            ..start
        });
        let standing = match (at, self.caret()?, self.selection()?) {
            (Some(at), Some(caret), None) if caret == at => char_marks(&self.doc, at.block)?
                .get(at.at - len..at.at)
                .is_some_and(|marks| marks.iter().all(|m| m.link.as_deref() == Some(url.as_str()))),
            _ => false,
        };
        if !standing {
            self.offer = None;
            return Ok(None);
        }
        Ok(at.map(|at| BookmarkOffer {
            at,
            len,
            url,
            selected,
        }))
    }

    /// Up or Down in the offer: moves the highlight, wrapping around.
    pub fn bookmark_offer_move(&mut self, direction: Vertical) {
        if let Some(offer) = &mut self.offer {
            let n = BOOKMARK_OFFER_CHOICES.len();
            offer.selected = match direction {
                Vertical::Down => (offer.selected + 1) % n,
                Vertical::Up => (offer.selected + n - 1) % n,
            };
        }
    }

    /// The mouse over entry `index`: highlights it.
    pub fn bookmark_offer_highlight(&mut self, index: usize) {
        if let Some(offer) = &mut self.offer
            && index < BOOKMARK_OFFER_CHOICES.len()
        {
            offer.selected = index;
        }
    }

    /// Escape, or any key the offer doesn't use: keeps the link and closes the offer.
    pub fn dismiss_bookmark_offer(&mut self) {
        self.offer = None;
    }

    /// Enter, or a click on entry `index` (`None` for the highlighted one). "Dismiss" just closes the
    /// offer; "Create web bookmark" turns the link into a bookmark: a block that holds only the link
    /// becomes the bookmark, with the caret on a new line below; otherwise the link's words go and the
    /// bookmark goes below the block (below the table, for a cell). One undo step. Returns where the
    /// caret went, or `None` with no offer standing.
    pub fn bookmark_offer_apply(&mut self, index: Option<usize>) -> Result<Option<Caret>, EditorError> {
        let Some(offer) = self.bookmark_offer()? else {
            return Ok(None);
        };
        self.offer = None;
        if index.unwrap_or(offer.selected) == 0 {
            return Ok(Some(offer.at));
        }
        let len = offer.len;
        let start = Caret {
            at: offer.at.at - len,
            ..offer.at
        };
        self.begin()?;
        let url = offer.url;
        let bookmark = BlockKind::WebBookmark {
            url: url.clone(),
            preview: WebPreview::default(),
        };
        let (caret, made) = self
            .doc
            .as_separate_step(|doc| place_bookmark(doc, start, len, bookmark))?;
        self.want_preview(made, &url);
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }

    /// Remembers that an address was asked for at `caret`, so the UI shows the prompt.
    pub(crate) fn request_web_address(&mut self, caret: Caret) -> Result<(), EditorError> {
        self.address = Some(AddressRequest {
            at: self.doc.anchor(caret)?,
            asked: false,
        });
        Ok(())
    }

    /// Whether the UI should show the address prompt now: true once per "Web bookmark" chosen in the
    /// slash menu.
    pub fn take_web_address_request(&mut self) -> bool {
        match &mut self.address {
            Some(request) if !request.asked => {
                request.asked = true;
                true
            }
            _ => false,
        }
    }

    /// Where the asked-for bookmark will go, for placing the prompt; `None` with no request.
    pub fn web_address_request_at(&self) -> Result<Option<Caret>, EditorError> {
        match &self.address {
            Some(request) => Ok(self.doc.resolve(&request.at)?),
            None => Ok(None),
        }
    }

    /// The prompt was closed without an address: forget the request.
    pub fn cancel_web_address(&mut self) {
        self.address = None;
    }

    /// The address typed into the prompt: an address without a scheme (`leaf.app/blog`) gets
    /// `https://`. `None` when it can't be a web bookmark's, so the prompt can say so.
    #[must_use]
    pub fn web_address(typed: &str) -> Option<String> {
        openable_url(typed).filter(|url| check_web_address(url).is_ok())
    }

    /// Adds a web bookmark for `typed` where it was asked for, else at the caret: an empty text block
    /// becomes the bookmark, with the caret on a new line below; otherwise the bookmark goes below the
    /// block (below the table, for a cell) and the caret stays. One undo step. Returns the caret, or
    /// `None` when the address can't be a bookmark's or there is nowhere to put it.
    pub fn insert_web_bookmark(&mut self, typed: &str) -> Result<Option<Caret>, EditorError> {
        let Some(url) = Self::web_address(typed) else {
            return Ok(None);
        };
        let requested = match self.address.take() {
            Some(request) => self.doc.resolve(&request.at)?,
            None => None,
        };
        let Some(target) = requested.or(self.caret()?) else {
            return Ok(None);
        };
        self.begin()?;
        let bookmark = BlockKind::WebBookmark {
            url: url.clone(),
            preview: WebPreview::default(),
        };
        let (caret, made) = self
            .doc
            .as_separate_step(|doc| place_bookmark(doc, target, 0, bookmark))?;
        self.want_preview(made, &url);
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }
}

impl PageEditor {
    /// The block menu's swap between a web bookmark and a link: "Turn into link" on a bookmark, "Turn
    /// into web bookmark" on a text block that is only one link to a web address; `None` otherwise.
    pub(crate) fn link_swap(
        &self,
        block: BlockId,
        kind: &BlockKind,
    ) -> Result<Option<crate::MenuEntry>, EditorError> {
        if matches!(kind, BlockKind::WebBookmark { .. }) {
            return Ok(Some(crate::MenuEntry::ToLink));
        }
        if !kind.allows_marks() || kind.is_table_part() || matches!(kind, BlockKind::Image { .. }) {
            return Ok(None);
        }
        Ok(only_link(&self.doc, block)?
            .filter(|url| check_web_address(url).is_ok())
            .map(crate::MenuEntry::ToWebBookmark))
    }
}

/// The address `block`'s whole text links to, when it is one link and nothing else.
pub(crate) fn only_link(doc: &PageDoc, block: BlockId) -> Result<Option<String>, EditorError> {
    let marks = char_marks(doc, block)?;
    let Some(url) = marks.first().and_then(|m| m.link.clone()) else {
        return Ok(None);
    };
    Ok(marks
        .iter()
        .all(|m| m.link.as_deref() == Some(url.as_str()) && m.page.is_none())
        .then_some(url))
}

/// Puts `bookmark` in place of the `len` characters at `start`: the whole block when that empties it
/// and it can change kind, else a new block below. Returns where the caret goes and the bookmark.
fn place_bookmark(
    doc: &mut PageDoc,
    start: Caret,
    len: usize,
    bookmark: BlockKind,
) -> Result<(Caret, BlockId), EditorError> {
    let block = start.block;
    let text = doc.plain_text(block)?.unwrap_or_default();
    let whole = start.at == 0 && text.chars().count() == len;
    let kind = doc.kind(block)?;
    let convertible = kind.has_text() && !kind.is_table_part() && !matches!(kind, BlockKind::Image { .. });
    if whole && convertible {
        // Turned whole: the link's words are the bookmark's address, so the model takes them as they are.
        doc.apply(PageCommand::SetBlockKind {
            id: block,
            kind: bookmark,
        })?;
        let caret = Caret {
            block: insert_after(doc, block, BlockKind::Paragraph)?,
            at: 0,
        };
        return Ok((caret, block));
    }
    if len > 0 {
        doc.apply(PageCommand::DeleteText {
            id: block,
            range: start.at..start.at + len,
        })?;
    }
    let below: BlockId = crate::table::table_of(doc, block)?.unwrap_or(block);
    let made = insert_after(doc, below, bookmark)?;
    Ok((start, made))
}

#[cfg(test)]
mod tests;
