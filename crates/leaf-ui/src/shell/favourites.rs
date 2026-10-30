//! The sidebar's Favourites section (E18-S05): the starred pages, above the page tree, so each is one
//! click away. Hidden while nothing is starred. The rows come from `WorkspaceEditor::favourites`, so
//! they follow renames, icon changes, the trash and restoring as soon as those happen.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_component::menu::ContextMenuExt as _;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use leaf_editor::{MenuPlace, SidebarItem};

use super::page_menu::page_menu;
use super::{Shell, report};

/// Height of a row, in pixels: the same as the page tree's.
const ROW_HEIGHT: f32 = 30.0;

/// Room on the left of a row's icon where the tree has its chevron, so both sections line up.
const CHEVRON_SLOT: f32 = 22.0;

impl Shell {
    /// The "Favourites" heading and one row per starred page, or nothing when none are starred.
    pub(super) fn render_favourites(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let items = self.editor.favourites().unwrap_or_else(|err| {
            report("could not list the favourites", err);
            Vec::new()
        });
        if items.is_empty() {
            return None;
        }
        let muted = cx.theme().muted_foreground;
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(ix, item)| self.render_favourite_row(ix, item, cx))
            .collect();
        let section = v_flex()
            .id("favourites")
            .debug_selector(|| "favourites".into())
            .child(
                div()
                    .h(px(28.0))
                    .mt_2()
                    .pl_3()
                    .flex()
                    .items_center()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("Favourites"),
            )
            .child(v_flex().px_2().children(rows));
        Some(section.into_any_element())
    }

    /// One starred page: its icon and title; a click opens it, a right-click shows its menu.
    fn render_favourite_row(&mut self, ix: usize, item: &SidebarItem, cx: &mut Context<Self>) -> AnyElement {
        let page = item.page;
        let theme = cx.theme();
        let (hover, active, radius) = (
            theme.sidebar_accent.opacity(0.6),
            theme.sidebar_accent,
            theme.radius,
        );
        let renaming = self.rename_field(page, true).is_some();
        let shell = cx.entity().downgrade();
        h_flex()
            .id(("favourite-row", ix))
            .debug_selector(|| format!("favourite-row-{ix}"))
            .w_full()
            .h(px(ROW_HEIGHT))
            .pl(px(4.0 + CHEVRON_SLOT))
            .pr_1()
            .gap_1()
            .rounded(radius)
            .text_sm()
            .hover(|style| style.bg(hover))
            .when(item.selected, |row| row.bg(active))
            .child(Self::page_icon(item, cx))
            .child(self.label(item, true, cx))
            .when(!renaming, |row| {
                // While renaming, clicks belong to the text field.
                row.cursor_pointer()
                    .on_click(cx.listener(move |shell, _, window, cx| shell.open(page, window, cx)))
            })
            .context_menu(move |menu, _, cx| page_menu(menu, &shell, page, MenuPlace::Favourites, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
