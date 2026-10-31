//! The sidebar's frame around the page tree: the workspace header with its compose button, the
//! search field (`sidebar_search`), the starred pages (`favourites`), the "Pages" heading with its "+", and Settings at the foot. Quiet icon buttons with tooltips instead
//! of labelled buttons, so the pages stay the loudest thing in the sidebar.

use std::path::Path;

use gpui::{
    AnyElement, App, ClickEvent, Context, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, SharedString, Styled, div, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::menu::{DropdownMenu as _, PopupMenu};
use gpui_component::sidebar::SidebarHeader;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;

use super::Shell;
use crate::menus::{NewWorkspace, OpenRecent, OpenWorkspace, SwitchWorkspace};
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
        let search = Self::render_sidebar_search(cx);
        let favourites = self.render_favourites(cx);
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
            .child(search)
            .children(favourites)
            .child(heading)
            .child(v_flex().flex_1().min_h_0().px_2().child(tree))
            .child(Self::render_footer(cx))
            .into_any_element()
    }

    /// The workspace switcher (badge, name and a ⇕ mark; clicking opens the workspace menu), the
    /// collapse button (`sidebar_toggle`) and the compose button that starts a new page.
    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let name = self.workspace_name();
        let initial: SharedString = name
            .chars()
            .next()
            .map_or_else(|| "L".into(), |c| c.to_uppercase().to_string().into());
        let theme = cx.theme();
        let (accent, radius, muted) = (theme.primary, theme.radius, theme.muted_foreground);
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
        let switcher = SidebarHeader::new()
            .debug_selector(|| "workspace-switcher".into())
            .px_1p5()
            .py_1()
            .justify_start()
            .child(badge)
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name),
            )
            .child(
                Icon::new(IconName::ChevronsUpDown)
                    .xsmall()
                    .text_color(muted)
                    .flex_none(),
            )
            .dropdown_menu({
                let focus = self.focus.clone();
                let current = self.storage.as_ref().map(|storage| storage.root().to_owned());
                move |menu, _, cx| workspace_menu(menu, &focus, current.as_deref(), cx)
            });
        let compose = crate::theme::button("new-page")
            .debug_selector(|| "new-page".into())
            .icon(Icon::new(IconName::SquarePen))
            .ghost()
            .small()
            .tooltip("New page")
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_page(window, cx)));
        h_flex()
            .h(px(48.0))
            .pl_1p5()
            .pr_2()
            .gap_1()
            // The menu wraps the switcher in a popover of its own, which must grow, not the switcher.
            .child(div().flex_1().min_w_0().child(switcher))
            .child(Self::render_collapse_button(cx))
            .child(compose)
            .into_any_element()
    }

    /// "Pages", with a "+" that shows when the pointer is over the heading and adds a top-level page.
    fn render_pages_heading(cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let add = crate::theme::button("add-page")
            .debug_selector(|| "add-page".into())
            .icon(Icon::new(IconName::Plus))
            .ghost()
            .xsmall()
            .tooltip("Add a page")
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_page(window, cx)));
        let add_database = crate::theme::button("add-database")
            .debug_selector(|| "add-database".into())
            .icon(Icon::new(IconName::Table))
            .ghost()
            .xsmall()
            .tooltip("Add a database")
            .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| shell.new_database(window, cx)));
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
                    .flex()
                    .child(add_database)
                    .child(add),
            )
            .into_any_element()
    }

    /// Trash and Settings, at the foot of the sidebar out of the way of the pages: two quiet rows, as
    /// compact as the page rows above them.
    fn render_footer(cx: &mut Context<Self>) -> AnyElement {
        let trash = Self::footer_row("open-trash", IconName::Trash, "Trash", cx).on_mouse_down(
            MouseButton::Left,
            cx.listener(|shell, _: &MouseDownEvent, window, cx| shell.open_trash(window, cx)),
        );
        let settings = Self::footer_row("open-settings", IconName::Settings, "Settings", cx).on_mouse_down(
            MouseButton::Left,
            cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                shell.open_settings(Section::Appearance, window, cx);
            }),
        );
        v_flex()
            .px_2()
            .pb_2()
            .child(trash)
            .child(settings)
            .into_any_element()
    }

    /// One foot row: icon and label, highlighted under the pointer.
    fn footer_row(
        id: &'static str,
        icon: IconName,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (hover, radius) = (cx.theme().sidebar_accent, cx.theme().radius);
        h_flex()
            .id(id)
            .debug_selector(move || id.to_owned())
            .cursor_pointer()
            .h(px(30.0))
            .px_2()
            .gap_2()
            .rounded(radius)
            .text_sm()
            .hover(|style| style.bg(hover.opacity(0.6)))
            .child(Icon::new(icon).small())
            .child(label)
    }
}

/// The workspace menu under the sidebar's workspace name: the recent workspaces (this one ticked),
/// then New, Open and All workspaces with their shortcuts. Its items are the File menu's actions,
/// sent from the workspace view so the app view (which switches workspaces) receives them.
fn workspace_menu(menu: PopupMenu, focus: &FocusHandle, current: Option<&Path>, cx: &App) -> PopupMenu {
    let recent = crate::settings::get(cx).recent_workspaces(Path::exists);
    let mut menu = menu
        .action_context(focus.clone())
        .min_w(px(240.0))
        .max_w(px(320.0))
        .label("Workspaces");
    for entry in recent.into_iter().filter(|entry| !entry.missing) {
        let here = current.is_some_and(|current| same_folder(current, &entry.path));
        menu = menu.menu_with_check(entry.name, here, Box::new(OpenRecent { path: entry.path }));
    }
    menu.separator()
        .menu("New workspace…", Box::new(NewWorkspace))
        .menu("Open workspace…", Box::new(OpenWorkspace))
        .menu("All workspaces…", Box::new(SwitchWorkspace))
}

/// Whether two paths name the same folder. The recent list holds resolved paths, while the open
/// workspace keeps the path it was opened with (from the command line it may go through a link, as
/// macOS's `/var` → `/private/var` does), so both are resolved before comparing.
fn same_folder(a: &Path, b: &Path) -> bool {
    let resolve = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    resolve(a) == resolve(b)
}

#[cfg(test)]
mod tests;
