//! The open page as a virtual list: only the rows on screen are laid out and drawn, so a page of ten
//! thousand blocks scrolls as smoothly as a page of ten (ADR-035).
//!
//! GPUI's `list` remembers each item's measured height by position. When the page changes, the list
//! must be told which positions changed, or it would keep stale heights and jump. [`changes`] works
//! that out by comparing the rows drawn last frame with this frame's: rows that only changed content
//! are re-measured in place, and the run of rows that were added, removed or moved is replaced,
//! keeping the scroll position everywhere else.

use std::ops::Range;

use gpui::{ListAlignment, ListState, px};
use leaf_core::{BlockId, PageId};
use leaf_editor::BlockRow;

/// Items before the first block row: the page title.
pub(super) const HEADER_ITEMS: usize = 1;

/// The list state for the open page, and the rows it was last told about.
pub(super) struct PageList {
    pub(super) state: ListState,
    pub(super) rows: Vec<BlockRow>,
    page: Option<PageId>,
    /// A scroll asked for before the list caught up with the page (right after opening it); applied
    /// on the next sync, after any reset.
    pending_scroll: Option<usize>,
}

impl Default for PageList {
    fn default() -> Self {
        // Lay out a screen's worth beyond the visible area, so a fast scroll doesn't show gaps.
        Self {
            state: ListState::new(HEADER_ITEMS, ListAlignment::Top, px(600.0)),
            rows: Vec::new(),
            page: None,
            pending_scroll: None,
        }
    }
}

impl PageList {
    /// Brings the list up to date with `rows` of `page`.
    pub(super) fn sync(&mut self, page: Option<PageId>, rows: Vec<BlockRow>) {
        if page == self.page {
            let change = changes(&self.rows, &rows);
            for index in change.remeasure {
                let item = HEADER_ITEMS + index;
                self.state.remeasure_items(item..item + 1);
            }
            if let Some((range, count)) = change.splice {
                self.state
                    .splice(range.start + HEADER_ITEMS..range.end + HEADER_ITEMS, count);
            }
        } else {
            // Another page: start from the top.
            self.state.reset(HEADER_ITEMS + rows.len());
            self.page = page;
        }
        self.rows = rows;
        if let Some(index) = self.pending_scroll.take() {
            self.state.scroll_to(gpui::ListOffset {
                item_ix: HEADER_ITEMS + index.min(self.rows.len().saturating_sub(1)),
                offset_in_item: px(0.0),
            });
        }
    }

    /// Scrolls so block number `index` (reading order, from 0) is at the top, once the list is in
    /// step with the page.
    pub(super) fn scroll_to_block(&mut self, index: usize) {
        self.pending_scroll = Some(index);
    }

    /// The row drawn as list item `item`, if it is a block.
    pub(super) fn row(&self, item: usize) -> Option<&BlockRow> {
        item.checked_sub(HEADER_ITEMS).and_then(|i| self.rows.get(i))
    }

    /// Scrolls so the row of `block` is on screen, and leaves the page alone if it already is.
    ///
    /// GPUI's `scroll_to_reveal_item` lines a row up with the bottom of the view even when the row is
    /// in view, which can scroll *up*: with the top of the window inside a tall first item (a page
    /// cover above the title), clicking any block jumped to the very top (E17-S06). A row taller than
    /// the view that is partly on screen counts as shown, so clicking in a long paragraph doesn't move
    /// the page either.
    pub(super) fn reveal(&self, block: BlockId) {
        let Some(i) = self.rows.iter().position(|row| row.id == block) else {
            return;
        };
        let item = HEADER_ITEMS + i;
        if let Some(row) = self.state.bounds_for_item(item) {
            let view = self.state.viewport_bounds();
            let inside = row.top() >= view.top() && row.bottom() <= view.bottom();
            let covers_view =
                row.top() < view.bottom() && row.bottom() > view.top() && row.size.height > view.size.height;
            if inside || covers_view {
                return;
            }
        }
        self.state.scroll_to_reveal_item(item);
    }
}

/// What changed between two frames' rows.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Changes {
    /// Rows at the same position with the same block, whose content changed (height may differ).
    pub(super) remeasure: Vec<usize>,
    /// Old rows `range` replaced by `count` new ones: the blocks added, removed or moved.
    pub(super) splice: Option<(Range<usize>, usize)>,
}

/// Compares the rows of the last frame with this frame's. The blocks that stayed at the start and at
/// the end are matched up; whatever lies between is spliced.
pub(super) fn changes(old: &[BlockRow], new: &[BlockRow]) -> Changes {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a.id == b.id).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(room)
        .take_while(|(a, b)| a.id == b.id)
        .count();
    let mut remeasure: Vec<usize> = (0..prefix).filter(|&i| old[i] != new[i]).collect();
    let (old_tail, new_tail) = (old.len() - suffix, new.len() - suffix);
    // Rows in the matched tail keep their old positions in the list until the splice is applied,
    // which happens after re-measuring.
    remeasure.extend(
        (0..suffix)
            .filter(|&k| old[old_tail + k] != new[new_tail + k])
            .map(|k| old_tail + k),
    );
    let splice = (prefix < old_tail || prefix < new_tail).then(|| (prefix..old_tail, new_tail - prefix));
    Changes { remeasure, splice }
}

#[cfg(test)]
mod tests;
