//! The slash menu's popover (E07-S06, ADR 0043): the list `PageEditor` keeps while a `/` query is
//! open, drawn under the `/` (or above it when there is no room below). The page keeps the keyboard,
//! so typing goes on filtering and the editor's keys (Up, Down, Enter, Escape) drive it; the mouse
//! highlights and picks entries. Nothing here decides anything.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, Styled, Window, anchored, deferred, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{SlashAction, SlashMenu};

use super::block_menu::kind_icon;
use super::{Shell, report};

/// One entry's height, and the panel's padding and border, to know how tall it will be before it is
/// laid out.
const ROW_PX: f32 = 30.0;
const CHROME_PX: f32 = 10.0;
/// Space between the line and the popover, and kept clear at the window's edges.
const GAP_PX: f32 = 4.0;
const MARGIN_PX: f32 = 8.0;

/// Which side of the line the popover opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    /// Its top edge just under the line.
    Below,
    /// Its bottom edge just over the line.
    Above,
}

/// Where a popover `height` tall goes for a line from `top` to `bottom` in a window `window` tall:
/// below when it fits, otherwise above when that fits, otherwise on the roomier side. Returns the
/// side and the y of its edge nearest the line.
pub(super) fn place(top: Pixels, bottom: Pixels, height: Pixels, window: Pixels) -> (Side, Pixels) {
    let (gap, margin) = (px(GAP_PX), px(MARGIN_PX));
    let below = window - margin - (bottom + gap);
    let above = top - gap - margin;
    if height <= below || (height > above && below >= above) {
        (Side::Below, bottom + gap)
    } else {
        (Side::Above, top - gap)
    }
}

impl Shell {
    /// The open slash menu as a popover by the `/`, while the page has the keyboard.
    pub(super) fn render_slash_popover(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.page_focus.is_focused(window) {
            return None;
        }
        let menu = self.page()?.slash_menu().ok().flatten()?;
        let (origin, line) = self.blocks.painted.caret_bounds(menu.at.block, menu.at.at)?;
        let rows = menu.items.len().max(1);
        let height = px(ROW_PX * f32::from(u16::try_from(rows).unwrap_or(u16::MAX)) + CHROME_PX);
        let (side, y) = place(origin.y, origin.y + line, height, window.viewport_size().height);
        let at: Point<Pixels> = point(origin.x - px(8.0), y);
        let corner = match side {
            Side::Below => gpui::Anchor::TopLeft,
            Side::Above => gpui::Anchor::BottomLeft,
        };
        Some(
            deferred(
                anchored()
                    .position(at)
                    .anchor(corner)
                    .snap_to_window_with_margin(px(MARGIN_PX))
                    .child(Self::render_slash_list(&menu, cx)),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn render_slash_list(menu: &SlashMenu, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (bg, border, accent, muted, radius) = (
            theme.popover,
            theme.border,
            theme.accent,
            theme.muted_foreground,
            theme.radius,
        );
        let empty = menu.items.is_empty().then(|| {
            div()
                .h(px(ROW_PX))
                .px_2()
                .flex()
                .items_center()
                .text_color(muted)
                .child("No results")
        });
        v_flex()
            .id("slash-menu")
            .debug_selector(|| "slash-menu".into())
            .min_w(px(220.0))
            .p_1()
            .bg(bg)
            .border_1()
            .border_color(border)
            .rounded(radius)
            .shadow_lg()
            .text_sm()
            // Clicks on the popover stay in it: they must not move the caret out of the query.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(empty)
            .children(menu.items.iter().enumerate().map(|(ix, item)| {
                h_flex()
                    .id(("slash-item", ix))
                    .debug_selector(move || format!("slash-item-{ix}"))
                    .h(px(ROW_PX))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(ix == menu.selected, |row| row.bg(accent))
                    .child(
                        Icon::new(match &item.action {
                            SlashAction::Block(kind) => kind_icon(kind),
                            SlashAction::Image => IconName::Image,
                        })
                        .small()
                        .text_color(muted),
                    )
                    .child(item.label)
                    .on_mouse_move(cx.listener(move |shell, _, _, cx| {
                        let changed = shell.page().and_then(|page| {
                            let before = page.slash_menu().ok().flatten()?.selected;
                            (before != ix).then(|| page.slash_highlight(ix))
                        });
                        if changed.is_some() {
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            if let Some(Err(err)) = shell.page().map(|page| page.slash_apply(Some(ix))) {
                                report("could not apply the slash menu", err);
                            }
                            shell.focus_page(window, cx);
                            shell.pick_image_if_asked(window, cx);
                        }),
                    )
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
