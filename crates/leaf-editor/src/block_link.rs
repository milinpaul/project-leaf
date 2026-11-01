//! Making a link to a block (E10-S08, ADR-XXX-E10-S08): "Copy link to block" in the block menu asks for
//! one, and the clipboard carries it as a fragment holding the link, which pastes into any page. Like any
//! page link, the copy names it for other apps: "Title › words" as plain text, `[[Title › words]]` in
//! Markdown.

use leaf_core::{BlockId, PageId};
use leaf_model::{Marks, PAGE_LINK, TextSpan};

use crate::{Fragment, PageEditor};

impl PageEditor {
    /// The block "Copy link to block" was chosen for, once: the UI calls it after the menu closes and
    /// copies [`Fragment::block_link`] for it.
    pub fn take_link_to_copy(&mut self) -> Option<BlockId> {
        self.link_to_copy.take()
    }
}

impl Fragment {
    /// Text holding one link to `block` inside `page`: what "Copy link to block" copies. It pastes at the
    /// caret, inside the line.
    #[must_use]
    pub fn block_link(page: PageId, block: BlockId) -> Self {
        Self::Text(vec![TextSpan {
            text: PAGE_LINK.to_string(),
            marks: Marks {
                page: Some(page),
                block: Some(block),
                ..Marks::default()
            },
        }])
    }
}

#[cfg(test)]
mod tests;
