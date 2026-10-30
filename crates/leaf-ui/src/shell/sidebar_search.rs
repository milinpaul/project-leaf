//! The search field at the top of the sidebar (E06-S17): a rounded box reading "Search" with the
//! shortcut at its right, like Notion's. It is a doorway, not a second search: it never holds text,
//! and clicking it opens the search bar exactly as `Cmd/Ctrl+K` does. The hint is drawn from the same
//! keys the shortcut is bound to (`menus::SEARCH_KEYS`), so it shows `⌘K` on macOS and `Ctrl+K`
//! elsewhere.

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, Keystroke, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_component::kbd::Kbd;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;

use super::Shell;
use crate::menus::SEARCH_KEYS;

impl Shell {
    /// The search field: icon, "Search" and the shortcut hint, highlighted under the pointer.
    pub(super) fn render_sidebar_search(cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, radius, border, hover) = (
            theme.muted_foreground,
            theme.radius,
            theme.sidebar_border,
            theme.sidebar_accent,
        );
        let hint = Keystroke::parse(SEARCH_KEYS)
            .ok()
            .map(|keys| Kbd::new(keys).outline());
        let field = h_flex()
            .id("sidebar-search")
            .debug_selector(|| "sidebar-search".into())
            .h(px(30.0))
            .px_2()
            .gap_2()
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(theme.background.opacity(0.6))
            .text_sm()
            .text_color(muted)
            .cursor_pointer()
            .hover(|style| style.bg(hover.opacity(0.6)))
            .child(Icon::new(IconName::Search).small().flex_none())
            .child(div().flex_1().min_w_0().truncate().child("Search"))
            .children(hint)
            // On the click, not the press: the press would take focus back from the search field.
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.open_search(window, cx)));
        div().px_2().pb_1().child(field).into_any_element()
    }
}
