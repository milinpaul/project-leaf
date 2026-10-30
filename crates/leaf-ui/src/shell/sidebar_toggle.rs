//! Hiding and showing the sidebar with buttons (E06-S18), as well as `Cmd/Ctrl+\` and View → Toggle
//! Sidebar: a collapse button in the sidebar's header, and while the sidebar is hidden a ☰ button in
//! the page's top-left corner that brings it back. The page top bar (E18-S03) will give that button a
//! place of its own.
//! The hidden state is an app setting, so it is remembered for the next launch.

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement, Styled, Window,
    div, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit_assets::IconName;

use super::Shell;
use crate::menus::ToggleSidebar;
use crate::settings;

/// How far the ☰ button sits from the page's top-left corner: level with where the sidebar header's
/// buttons were.
const REOPEN_INSET: f32 = 10.0;

impl Shell {
    /// Hides the sidebar if it shows, shows it if hidden. From a button, the page then gets the
    /// keyboard back, as if the click had never taken it; from the shortcut, focus stays put.
    pub fn toggle_sidebar(&mut self, refocus: bool, window: &mut Window, cx: &mut Context<Self>) {
        settings::update(cx, |settings| {
            settings.set_sidebar_hidden(!settings.sidebar_hidden());
        });
        if refocus && self.editor.selected().is_some() {
            self.focus_page(window, cx);
        }
        cx.notify();
    }

    /// The collapse button in the sidebar's header; its tooltip names the shortcut.
    pub(super) fn render_collapse_button(cx: &mut Context<Self>) -> Button {
        Button::new("collapse-sidebar")
            .debug_selector(|| "collapse-sidebar".into())
            .icon(Icon::new(IconName::PanelLeft))
            .ghost()
            .small()
            .tooltip_with_action("Hide sidebar", &ToggleSidebar, None)
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.toggle_sidebar(true, window, cx)))
    }

    /// The ☰ button that brings the sidebar back, floating in the page's top-left corner while the
    /// sidebar is hidden. It floats rather than taking a row, so the page doesn't jump down when the
    /// sidebar goes; a soft backing keeps it legible over a cover image.
    pub(super) fn render_reopen_button(cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (backing, radius) = (theme.background.opacity(0.85), theme.radius);
        let open = Button::new("show-sidebar")
            .debug_selector(|| "show-sidebar".into())
            .icon(Icon::new(IconName::Menu))
            .ghost()
            .small()
            .tooltip_with_action("Show sidebar", &ToggleSidebar, None)
            .on_click(
                cx.listener(|shell, _: &ClickEvent, window, cx| shell.toggle_sidebar(true, window, cx)),
            );
        div()
            .absolute()
            .top(px(REOPEN_INSET))
            .left(px(REOPEN_INSET))
            .rounded(radius)
            .bg(backing)
            .child(open)
            .into_any_element()
    }
}
