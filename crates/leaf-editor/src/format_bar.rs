//! The floating formatting bar over selected text (E07-S13): what it offers for the current selection.
//!
//! The bar shows for any text selection, in one block or across several. It holds a "Turn into" list
//! and buttons for the marks; this module says which of them apply and which are on, so the UI only
//! draws them and forwards clicks to [`PageEditor::toggle_mark`], the link popover and
//! [`crate::BlockAction::TurnInto`], the same paths the keyboard and the block menu take.

use leaf_model::{BlockKind, Caret, MarkKind};

use crate::block_menu::{kind_label, same_kind};
use crate::{EditorError, PageEditor, Selection, turn_into_kinds};

/// What the bar shows for the current selection.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatBar {
    /// Where the selection starts, in reading order: the bar sits just above it.
    pub start: Caret,
    /// Where it ends: the bar goes below this line when there is no room above the start.
    pub end: Caret,
    /// The "Turn into" list, empty when a selected block can't change kind (a table cell, a block
    /// from a newer version).
    pub kinds: Vec<KindChoice>,
    /// What the "Turn into" button says: the kind every selected block shares, or "Turn into".
    pub kind_label: &'static str,
    /// Bold, italic, strikethrough, code and link, in that order; empty when no selected text can
    /// take marks (only code blocks are selected).
    pub marks: Vec<MarkButton>,
    /// Whether "Link to page" is offered: the selection is words in one block that can hold a page link,
    /// without a line break or `]`, so they can search the page list (E10-S10).
    pub link_to_page: bool,
}

/// One kind in the "Turn into" list.
#[derive(Debug, Clone, PartialEq)]
pub struct KindChoice {
    /// The kind every selected block becomes.
    pub kind: BlockKind,
    /// Its name.
    pub label: &'static str,
    /// Whether every selected block already is this kind.
    pub current: bool,
}

/// One mark button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkButton {
    /// The mark it toggles; [`MarkKind::Link`] opens the link popover instead.
    pub kind: MarkKind,
    /// Its name, for the tooltip.
    pub label: &'static str,
    /// Whether every selected character has the mark, so the button shows as on.
    pub active: bool,
}

impl PageEditor {
    /// The bar for the current selection, or `None` when there is no text selected (just a caret,
    /// whole blocks or table cells).
    pub fn format_bar(&mut self) -> Result<Option<FormatBar>, EditorError> {
        let Some(Selection::Text { start, end }) = self.selection()? else {
            return Ok(None);
        };
        if start == end {
            return Ok(None);
        }
        let mut blocks = Vec::new();
        for block in self.blocks_between(start, end)? {
            blocks.push(self.doc.kind(block)?);
        }
        let choices = turn_into_kinds();
        let changeable = blocks
            .iter()
            .all(|kind| choices.iter().any(|choice| same_kind(choice, kind)));
        let shared = blocks
            .first()
            .filter(|first| blocks.iter().all(|kind| same_kind(kind, first)));
        let kinds = if changeable {
            choices
                .into_iter()
                .map(|kind| KindChoice {
                    current: shared.is_some_and(|shared| same_kind(shared, &kind)),
                    label: kind_label(&kind),
                    kind,
                })
                .collect()
        } else {
            Vec::new()
        };
        let label = match shared {
            Some(kind) if changeable => kind_label(kind),
            _ => "Turn into",
        };
        let marks = if self.markable_ranges()?.is_empty() {
            Vec::new()
        } else {
            let on = self.active_marks()?;
            vec![
                button(MarkKind::Bold, "Bold", on.bold),
                button(MarkKind::Italic, "Italic", on.italic),
                button(MarkKind::Strike, "Strikethrough", on.strike),
                button(MarkKind::Code, "Code", on.code),
                button(MarkKind::Link, "Link", on.link.is_some()),
            ]
        };
        let link_to_page = !marks.is_empty() && self.selection_can_link(start, end)?;
        Ok(Some(FormatBar {
            start,
            end,
            kinds,
            kind_label: label,
            marks,
            link_to_page,
        }))
    }
}

fn button(kind: MarkKind, label: &'static str, active: bool) -> MarkButton {
    MarkButton { kind, label, active }
}

#[cfg(test)]
mod tests;
