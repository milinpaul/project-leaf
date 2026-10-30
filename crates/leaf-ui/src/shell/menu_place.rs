//! Where the ⋮⋮ block menu and its "Turn into" list go (E17-S07), worked out before they are drawn
//! so neither moves once open, like Notion's. The menu opens below its handle, or above it when there
//! is no room below (the slash menu's rule). The list opens beside the "Turn into" row, level with it
//! when it fits; otherwise it moves up only as far as it must to stay in the window, and scrolls when
//! it is taller than the window. At the window's right edge it opens on the menu's left instead.

use gpui::{Bounds, Pixels, Point, Size, point, px, size};

use super::slash_popover::{Side, place};

/// Kept clear at the window's edges, and between the menu and the list.
const MARGIN_PX: f32 = 8.0;
const GAP_PX: f32 = 4.0;

/// Where the menu goes: its top-left corner, for a menu `menu` in size hanging from a handle whose
/// left edge is `x` and which spans `top` to `bottom`, in a window `window` in size.
pub(super) fn menu_origin(
    x: Pixels,
    top: Pixels,
    bottom: Pixels,
    menu: Size<Pixels>,
    window: Size<Pixels>,
) -> Point<Pixels> {
    let (side, edge) = place(top, bottom, menu.height, window.height);
    let y = match side {
        Side::Below => edge,
        Side::Above => edge - menu.height,
    };
    let margin = px(MARGIN_PX);
    point(
        x.min(window.width - margin - menu.width).max(margin),
        y.max(margin),
    )
}

/// Where the "Turn into" list goes, beside the menu at `menu` (its bounds) with the "Turn into" row's
/// top at `row_top`, for a list `list` in size. The result's height is the list's, or less when it must
/// scroll.
pub(super) fn list_bounds(
    menu: Bounds<Pixels>,
    row_top: Pixels,
    list: Size<Pixels>,
    window: Size<Pixels>,
) -> Bounds<Pixels> {
    let (margin, gap) = (px(MARGIN_PX), px(GAP_PX));
    let right = menu.origin.x + menu.size.width + gap;
    let left = menu.origin.x - gap - list.width;
    let x = if right + list.width <= window.width - margin || left < margin {
        right
    } else {
        left
    };
    let room = window.height - margin * 2.0;
    let height = list.height.min(room);
    let y = row_top.min(window.height - margin - height).max(margin);
    Bounds::new(point(x, y), size(list.width, height))
}

#[cfg(test)]
mod tests;
