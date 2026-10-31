//! The page-link menu's popover (E10-S05, ADR 0071): the pages `WorkspaceEditor::link_menu` lists for a
//! `[[` query, drawn under the `[[` (or above it near the window's bottom edge), each with its icon, title
//! and where it lives, and "New page" last. The page keeps the keyboard, so typing goes on filtering and
//! Up, Down, Enter and Escape drive it; the mouse highlights and picks entries. Nothing here decides
//! anything.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, Styled, Window, anchored, deferred, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{LinkChoice, LinkMenu};

use super::slash_popover::{Side, place};
use super::{Shell, now_ms, report};

/// One entry's height, and the panel's padding and border, to know how tall it will be before it is laid
/// out.
const ROW_PX: f32 = 32.0;
const CHROME_PX: f32 = 10.0;
/// Kept clear at the window's edges.
const MARGIN_PX: f32 = 8.0;

impl Shell {
    /// The open page-link menu as a popover by the `[[`, while the page has the keyboard.
    pub(super) fn render_link_menu_popover(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.page_focus.is_focused(window) {
            return None;
        }
        let menu = self.editor.link_menu().ok().flatten()?;
        let (origin, line) = self.blocks.painted.caret_bounds(menu.at.block, menu.at.at)?;
        let rows = menu.choices.len().max(1);
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
                    .child(Self::render_link_list(&menu, cx)),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    fn render_link_list(menu: &LinkMenu, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (bg, border, accent, muted, radius) = (
            theme.popover,
            theme.border,
            theme.accent,
            theme.muted_foreground,
            theme.radius,
        );
        let empty = menu.choices.is_empty().then(|| {
            div()
                .h(px(ROW_PX))
                .px_2()
                .flex()
                .items_center()
                .text_color(muted)
                .child("Type a page's title")
        });
        v_flex()
            .id("link-menu")
            .debug_selector(|| "link-menu".into())
            .min_w(px(260.0))
            .max_w(px(420.0))
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
            .children(menu.choices.iter().enumerate().map(|(ix, choice)| {
                let (icon, label, path) = match choice {
                    LinkChoice::Page(result) => (
                        result.icon.clone().map_or_else(
                            || {
                                Icon::new(IconName::FileText)
                                    .small()
                                    .text_color(muted)
                                    .into_any_element()
                            },
                            |emoji| div().child(emoji).into_any_element(),
                        ),
                        result.label.clone(),
                        result.path.clone(),
                    ),
                    LinkChoice::NewPage(title) => (
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(muted)
                            .into_any_element(),
                        format!("New page \u{201c}{title}\u{201d}"),
                        String::new(),
                    ),
                };
                h_flex()
                    .id(("link-item", ix))
                    .debug_selector(move || format!("link-item-{ix}"))
                    .h(px(ROW_PX))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(ix == menu.selected, |row| row.bg(accent))
                    .child(div().w(px(18.0)).flex().justify_center().child(icon))
                    .child(div().flex_shrink_0().child(label))
                    .when(!path.is_empty(), |row| {
                        row.child(div().min_w_0().truncate().text_xs().text_color(muted).child(path))
                    })
                    .on_mouse_move(cx.listener(move |shell, _, _, cx| {
                        let before = shell.editor.link_menu().ok().flatten().map(|menu| menu.selected);
                        if before.is_some_and(|before| before != ix) {
                            let _ = shell.editor.link_menu_highlight(ix);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            shell.apply_link_menu(Some(ix), window, cx);
                        }),
                    )
            }))
            .into_any_element()
    }

    /// Links the entry `index` (`None`: the highlighted one) and keeps typing in the page.
    pub(super) fn apply_link_menu(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.editor.link_menu_apply(index, now_ms()) {
            report("could not link the page", err);
        }
        self.focus_page(window, cx);
        self.changed(window, cx);
    }
}

#[cfg(test)]
mod tests;
