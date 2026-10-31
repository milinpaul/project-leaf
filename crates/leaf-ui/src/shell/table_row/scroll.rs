//! A table's sideways scrolling (E16-S05): the horizontal scrollbar under its grid, shown while the pointer is
//! over the table, and bringing the caret's column into the box when Tab or an arrow moves it out of view.

use gpui::{InteractiveElement as _, ParentElement, Pixels, SharedString, Styled, div, point, px};
use gpui_component::scroll::{Scrollbar, ScrollbarMode};
use leaf_core::BlockId;

use super::super::Shell;
use super::{CELL_WIDTH, HANDLE};

impl Shell {
    /// The horizontal scrollbar under `table`'s grid, shown while the pointer is over the table.
    pub(super) fn table_scrollbar(table: BlockId, scroll: &gpui::ScrollHandle) -> gpui::Div {
        div()
            .debug_selector(move || format!("table-scrollbar-{table}"))
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(Scrollbar::width())
            .child(
                Scrollbar::horizontal(scroll)
                    .id(SharedString::from(format!("table-scrollbar-{table}")))
                    .mode(ScrollbarMode::Hover),
            )
    }

    /// Scrolls `table`'s box sideways so the caret, drawn at `x` (window coordinates) in one of its cells,
    /// is inside it with the rest of a column's width after it: Tab or an arrow into a column out of view
    /// brings that column in (E16-S05).
    pub(in crate::shell) fn reveal_table_column(&self, table: BlockId, x: Pixels) {
        let Some(scroll) = self.blocks.table_scroll.get(&table) else {
            return;
        };
        let frame = scroll.bounds();
        let offset = scroll.offset();
        let (left, right) = (frame.left() + px(HANDLE), frame.right() - px(CELL_WIDTH));
        let shift = if x < left {
            left - x
        } else if x > right {
            right - x
        } else {
            return;
        };
        let new = (offset.x + shift).clamp(-scroll.max_offset().x, px(0.0));
        if new != offset.x {
            scroll.set_offset(point(new, offset.y));
        }
    }
}
