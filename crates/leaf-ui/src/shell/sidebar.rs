//! The sidebar's frame around the page tree: the workspace header with its compose button, the
//! "Pages" heading with its "+", and Settings at the foot. Quiet icon buttons with tooltips instead
//! of labelled buttons, so the pages stay the loudest thing in the sidebar.

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, SharedString, Styled, div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::sidebar::SidebarFooter;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;

use super::Shell;
use crate::settings_dialog::Section;

/// The name shown for a workspace that isn't saved to a folder (tests).
const UNSAVED_NAME: &str = "Leaf";

impl Shell {
    /// The workspace's name: its folder name without the `.leaf` ending.
    pub(super) fn workspace_name(&self) -> String {
        self.storage
            .as_ref()
            .and_then(|storage| storage.root().file_stem())
            .map_or_else(
                || UNSAVED_NAME.to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            )
    }

    /// The whole sidebar column.
    pub(super) fn render_sidebar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (background, foreground, border) =
            (theme.sidebar, theme.sidebar_foreground, theme.sidebar_border);
        let header = self.render_header(cx);
        let heading = Self::render_pages_heading(cx);
        let tree = self.render_tree(cx);
        v_flex()
            .id("sidebar")
            .size_full()
            .bg(background)
            .text_color(foreground)
            .border_r_1()
            .border_color(border)
            .child(header)
            .child(heading)
            .child(v_flex().flex_1().min_h_0().px_2().child(tree))
            .child(Self::render_footer(cx))
            .into_any_element()
    }

    /// The workspace badge and name, and the compose button that starts a new page.
    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.workspace_name();
        let initial: SharedString = name
            .chars()
            .next()
            .map_or_else(|| "L".into(), |c| c.to_uppercase().to_string().into());
        let theme = cx.theme();
        let (accent, radius) = (theme.primary, theme.radius);
        let badge = div()
            .flex_none()
            .size(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(radius)
            .bg(accent.opacity(0.14))
            .text_color(accent)
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .child(initial);
        let compose = Button::new("new-page")
            .debug_selector(|| "new-page".into())
            .icon(Icon::new(IconName::SquarePen))
            .ghost()
            .small()
            .tooltip("New page")
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_page(window, cx)));
        h_flex()
            .h(px(48.0))
            .pl_3()
            .pr_2()
            .gap_2()
            .child(badge)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(compose)
            .into_any_element()
    }

    /// "Pages", with a "+" that shows when the pointer is over the heading and adds a top-level page.
    fn render_pages_heading(cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let add = Button::new("add-page")
            .debug_selector(|| "add-page".into())
            .icon(Icon::new(IconName::Plus))
            .ghost()
            .xsmall()
            .tooltip("Add a page")
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_page(window, cx)));
        h_flex()
            .id("pages-heading")
            .group("pages-heading")
            .h(px(28.0))
            .mt_2()
            .pl_3()
            .pr_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("Pages"),
            )
            .child(
                div()
                    .opacity(0.0)
                    .group_hover("pages-heading", |style| style.opacity(1.0))
                    .child(add),
            )
            .into_any_element()
    }

    /// Settings, at the foot of the sidebar out of the way of the pages, as a quiet row.
    fn render_footer(cx: &mut Context<Self>) -> AnyElement {
        SidebarFooter::new()
            .w_full()
            .gap_2()
            .text_sm()
            .child(Icon::new(IconName::Settings))
            .child("Settings")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                    shell.open_settings(Section::Appearance, window, cx);
                }),
            )
            .into_any_element()
    }
}
