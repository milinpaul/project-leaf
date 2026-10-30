//! The slash menu: typing `/` at the start of a word opens a list of block types, filtered as the user
//! keeps typing, to convert the block or add one below.
//!
//! The `/` and the query are ordinary typed text in the page, so they show while the menu is open and
//! stay if the menu closes without applying. The editor only remembers where the `/` is (an anchor,
//! so edits elsewhere don't lose it) and which entry is highlighted. Everything else is read from the
//! text each time, so the menu can never disagree with what is on the page (ADR 0026).

mod items;

pub use items::{SlashAction, SlashItem};

use leaf_core::BlockId;
use leaf_model::{Anchor, BlockKind, Caret, PageCommand, PageDoc};

use crate::caret::Vertical;
use crate::page_editor::{insert_after, insert_table};
use crate::{EditorError, PageEditor};

/// The open menu as the editor keeps it.
pub(crate) struct Slash {
    /// Where the `/` is.
    at: Anchor,
    /// The highlighted entry, as an index into the filtered list.
    selected: usize,
    /// The query the highlight belongs to; a new query moves the highlight back to the top.
    query: String,
}

/// The open slash menu, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct SlashMenu {
    /// Where the `/` is, to place the menu under it.
    pub at: Caret,
    /// What was typed after the `/`.
    pub query: String,
    /// The matching entries, best first.
    pub items: Vec<SlashItem>,
    /// Which entry is highlighted.
    pub selected: usize,
}

impl PageEditor {
    /// The open slash menu, if there is one. It closes by itself when the `/` is deleted, the caret
    /// leaves the query (moves before the `/` or to another block), or a space is typed with nothing
    /// matching.
    pub fn slash_menu(&mut self) -> Result<Option<SlashMenu>, EditorError> {
        let Some(slash) = &self.slash else {
            return Ok(None);
        };
        let anchor = slash.at.clone();
        let (Some(at), Some(caret)) = (self.doc.resolve(&anchor)?, self.caret()?) else {
            self.slash = None;
            return Ok(None);
        };
        let text: Vec<char> = self
            .doc
            .plain_text(at.block)?
            .unwrap_or_default()
            .chars()
            .collect();
        let open = caret.block == at.block && caret.at > at.at && text.get(at.at) == Some(&'/');
        if !open {
            self.slash = None;
            return Ok(None);
        }
        let query: String = text[at.at + 1..caret.at.min(text.len())].iter().collect();
        let items = items::filter(&items::all_items(), &query);
        let Some(slash) = &mut self.slash else {
            return Ok(None);
        };
        if slash.query != query {
            slash.query.clone_from(&query);
            slash.selected = 0;
        }
        let selected = slash.selected.min(items.len().saturating_sub(1));
        Ok(Some(SlashMenu {
            at,
            query,
            items,
            selected,
        }))
    }

    /// Up and Down while the menu is open: moves the highlight, wrapping around. Returns the menu.
    pub fn slash_move(&mut self, direction: Vertical) -> Result<Option<SlashMenu>, EditorError> {
        let Some(menu) = self.slash_menu()? else {
            return Ok(None);
        };
        let count = menu.items.len();
        if count > 0
            && let Some(slash) = &mut self.slash
        {
            slash.selected = match direction {
                Vertical::Up => (menu.selected + count - 1) % count,
                Vertical::Down => (menu.selected + 1) % count,
            };
        }
        self.slash_menu()
    }

    /// The mouse over entry `index`: highlights it. Returns the menu.
    pub fn slash_highlight(&mut self, index: usize) -> Result<Option<SlashMenu>, EditorError> {
        let Some(menu) = self.slash_menu()? else {
            return Ok(None);
        };
        if index < menu.items.len()
            && let Some(slash) = &mut self.slash
        {
            slash.selected = index;
        }
        self.slash_menu()
    }

    /// Escape: closes the menu, leaving the typed `/` and query as text.
    pub fn slash_close(&mut self) {
        self.slash = None;
    }

    /// Enter, or a click on entry `index` (`None` for the highlighted one): removes the `/` and query
    /// and applies the entry. An empty block is converted; otherwise the new block goes below it and
    /// the caret into it. A divider never takes the caret: it stays above, or goes to a new paragraph
    /// below when the divider replaced an empty block. One undo step of its own, so undo right after
    /// gives back the typed `/` and query. Returns where the caret went, or `None` with no menu.
    pub fn slash_apply(&mut self, index: Option<usize>) -> Result<Option<Caret>, EditorError> {
        let Some(menu) = self.slash_menu()? else {
            return Ok(None);
        };
        let Some(item) = menu.items.get(index.unwrap_or(menu.selected)).cloned() else {
            return Ok(None);
        };
        self.slash = None;
        self.begin()?;
        let block = menu.at.block;
        let typed = menu.at.at..menu.at.at + 1 + menu.query.chars().count();
        let kind = match item.action {
            SlashAction::Block(kind) => kind,
            SlashAction::Image => {
                // The `/image` goes now; the picture arrives once the owner has picked a file.
                self.doc.as_separate_step(|doc| {
                    doc.apply(PageCommand::DeleteText {
                        id: block,
                        range: typed.clone(),
                    })
                })?;
                let caret = self.set_caret(Caret {
                    block,
                    at: typed.start,
                })?;
                self.finish()?;
                self.request_image(caret)?;
                return Ok(Some(caret));
            }
            SlashAction::Table => {
                let caret = self
                    .doc
                    .as_separate_step(|doc| table_in_place_of(doc, block, typed.clone()))?;
                let caret = self.set_caret(caret)?;
                self.finish()?;
                return Ok(Some(caret));
            }
        };
        let caret = self.doc.as_separate_step(|doc| -> Result<Caret, EditorError> {
            doc.apply(PageCommand::DeleteText {
                id: block,
                range: typed.clone(),
            })?;
            let empty = doc.plain_text(block)?.is_some_and(|t| t.is_empty());
            let divider = kind == BlockKind::Divider;
            if empty {
                doc.apply(PageCommand::SetBlockKind {
                    id: block,
                    kind: kind.clone(),
                })?;
                return Ok(if divider {
                    Caret {
                        block: insert_after(doc, block, BlockKind::Paragraph)?,
                        at: 0,
                    }
                } else {
                    Caret { block, at: 0 }
                });
            }
            let below = insert_after(doc, block, kind)?;
            Ok(if divider {
                Caret {
                    block,
                    at: typed.start,
                }
            } else {
                Caret { block: below, at: 0 }
            })
        })?;
        let caret = self.set_caret(caret)?;
        self.finish()?;
        Ok(Some(caret))
    }

    /// After typing that left the caret at `caret`: opens the menu if the typing ended with a `/` at
    /// the start of a word. Code blocks and image captions take `/` literally.
    pub(crate) fn maybe_open_slash(&mut self, typed: &str, caret: Caret) -> Result<(), EditorError> {
        if !typed.ends_with('/') {
            // A space with nothing matching gives up on the menu.
            if typed.ends_with(' ') && self.slash_menu()?.is_some_and(|menu| menu.items.is_empty()) {
                self.slash = None;
            }
            return Ok(());
        }
        let kind = self.doc.kind(caret.block)?;
        // A cell can't turn into another kind of block, so `/` is just a character there.
        if !kind.allows_marks() || matches!(kind, BlockKind::Image { .. } | BlockKind::TableCell) {
            return Ok(());
        }
        let text: Vec<char> = self
            .doc
            .plain_text(caret.block)?
            .unwrap_or_default()
            .chars()
            .collect();
        let Some(slash_at) = caret.at.checked_sub(1) else {
            return Ok(());
        };
        let word_start = slash_at
            .checked_sub(1)
            .and_then(|i| text.get(i))
            .is_none_or(|c| c.is_whitespace());
        if word_start {
            self.slash = Some(Slash {
                at: self.doc.anchor(Caret {
                    block: caret.block,
                    at: slash_at,
                })?,
                selected: 0,
                query: String::new(),
            });
        }
        Ok(())
    }
}

/// The size `/table` makes: a header row and two rows under it, three columns.
const NEW_TABLE: (usize, usize) = (3, 3);

/// Removes the typed `/table` (`typed`) from `block` and adds a new table: in place of `block` when
/// that leaves it an empty paragraph with nothing nested in it, else right below it. Returns the start
/// of the first cell.
fn table_in_place_of(
    doc: &mut PageDoc,
    block: BlockId,
    typed: std::ops::Range<usize>,
) -> Result<Caret, EditorError> {
    doc.apply(PageCommand::DeleteText {
        id: block,
        range: typed,
    })?;
    let parent = doc.parent(block)?;
    let siblings = match parent {
        Some(parent) => doc.children(parent)?,
        None => doc.root_blocks(),
    };
    let index = siblings
        .iter()
        .position(|&b| b == block)
        .unwrap_or(siblings.len());
    let replace = doc.kind(block)? == BlockKind::Paragraph
        && doc.plain_text(block)?.is_none_or(|t| t.is_empty())
        && doc.children(block)?.is_empty();
    let table = insert_table(doc, parent, index + 1, NEW_TABLE, true)?;
    if replace {
        doc.apply(PageCommand::DeleteBlocks { ids: vec![block] })?;
    }
    let first = doc
        .table_cells(table)?
        .first()
        .and_then(|row| row.first().copied());
    match first {
        Some(block) => Ok(Caret { block, at: 0 }),
        None => Err(leaf_model::ModelError::EmptyTable.into()),
    }
}

#[cfg(test)]
mod tests;
