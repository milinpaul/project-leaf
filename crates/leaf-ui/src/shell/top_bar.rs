//! The thin bar across the top of the open page (E18-S03), like Notion's: on the left where the page
//! sits (its parent pages, then its own icon and title, each opening its page); on the right when it
//! was last edited, a star for Favourites, and the `⋯` page menu (`page_menu`). While the sidebar is hidden, its ☰
//! reopen button sits at the bar's left end. The breadcrumb and the words come from `leaf-editor`
//! (`WorkspaceEditor::breadcrumb`, `edited_ago`); this module only draws them.

use std::time::Duration;

use chrono::{Local, Offset as _, TimeZone as _};
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, div, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::menu::DropdownMenu as _;
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::PageId;
use leaf_editor::{Crumb, MenuPlace, edited_ago, edited_on};

use super::page_menu::page_menu;
use super::{Shell, now_ms, report};
use crate::settings;

/// The bar's height: the sidebar header's, so the two line up.
pub(super) const TOP_BAR_PX: f32 = 44.0;

/// How often the edited label is redrawn, so "Edited 3 minutes ago" keeps counting while the page
/// stays open. Twice a minute keeps it within half a minute of the truth.
const EDITED_REFRESH: Duration = Duration::from_secs(30);

/// How much faster the open page's title shortens than its ancestors' when the bar is short of room.
const TITLE_SHRINK: f32 = 50.0;

/// The widest an ancestor's crumb gets before it is shortened; the open page's own title takes the
/// rest, and is the first to shorten when the window is narrow.
const ANCESTOR_MAX_PX: f32 = 180.0;

impl Shell {
    /// Redraws the window every [`EDITED_REFRESH`] so the edited label stays current. Called once when
    /// the window is built.
    pub(super) fn start_edited_clock(cx: &mut Context<Self>) {
        cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor().timer(EDITED_REFRESH).await;
                if shell.update(cx, |_, cx| cx.notify()).is_err() {
                    break; // the window is gone
                }
            }
        })
        .detach();
    }

    /// The bar for the open page `page`.
    pub(super) fn render_top_bar(&mut self, page: PageId, cx: &mut Context<Self>) -> AnyElement {
        let crumbs = self.editor.breadcrumb(page).unwrap_or_else(|err| {
            report("could not work out where the page sits", err);
            Vec::new()
        });
        let sidebar_hidden = settings::get(cx).sidebar_hidden();
        h_flex()
            .id("top-bar")
            .debug_selector(|| "top-bar".into())
            .flex_none()
            .w_full()
            .h(px(TOP_BAR_PX))
            .px_2()
            .gap_1()
            .when(sidebar_hidden, |bar| bar.child(Self::reopen_sidebar_button(cx)))
            .child(Self::render_crumbs(&crumbs, cx))
            .child(self.render_edited(page, cx))
            .child(self.render_star(page, cx))
            .child(Self::render_more(page, cx))
            .into_any_element()
    }

    /// The breadcrumb: ancestors (each shortened past a set width), then the page, which takes the
    /// rest of the line and is shortened first.
    fn render_crumbs(crumbs: &[Crumb], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, hover, radius) = (theme.muted_foreground, theme.muted, theme.radius);
        let last = crumbs.len().saturating_sub(1);
        let mut row = h_flex()
            .debug_selector(|| "crumbs".into())
            .flex_1()
            .min_w_0()
            // Clipped at its edge: in a very narrow window nothing spills over the controls.
            .overflow_hidden()
            .gap_0p5()
            .text_sm();
        for (ix, crumb) in crumbs.iter().enumerate() {
            if ix > 0 {
                row = row.child(div().flex_none().text_color(muted).child("/"));
            }
            row = row.child(match crumb {
                Crumb::Gap => div()
                    .flex_none()
                    .px_1()
                    .text_color(muted)
                    .child("…")
                    .into_any_element(),
                Crumb::Page { page, label, icon } => {
                    let page = *page;
                    let text = match icon {
                        Some(icon) => format!("{icon} {label}"),
                        None => label.clone(),
                    };
                    div()
                        .id(SharedString::from(format!("crumb-{ix}")))
                        .debug_selector(move || format!("crumb-{ix}"))
                        .min_w_0()
                        .px_1p5()
                        .py_0p5()
                        .rounded(radius)
                        .truncate()
                        .cursor_pointer()
                        .hover(|style| style.bg(hover))
                        // The open page (last) shortens first; ancestors, capped in width, give way
                        // only far more slowly, so the controls on the right always stay in view.
                        .when(ix == last, |crumb| crumb.flex_shrink(TITLE_SHRINK))
                        .when(ix != last, |crumb| {
                            crumb.flex_shrink(1.0).max_w(px(ANCESTOR_MAX_PX))
                        })
                        .child(text)
                        .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                            shell.open(page, window, cx);
                        }))
                        .into_any_element()
                }
            });
        }
        row.into_any_element()
    }

    /// "Edited 3 minutes ago", with the exact local date and time on hover. A page with changes not
    /// yet stamped (the save timer stamps them within a tick) already reads "Edited just now".
    fn render_edited(&mut self, page: PageId, cx: &mut Context<Self>) -> AnyElement {
        let now = now_ms();
        let edited = self
            .editor
            .workspace()
            .page(page)
            .map_or(now, |meta| meta.edited_at);
        let pending = self
            .editor
            .page_editor(page)
            .is_some_and(|editor| editor.doc().has_unstamped_edits());
        let at = if pending { now } else { edited };
        let exact = edited_on(at, local_offset_minutes(at));
        div()
            .id("edited")
            .debug_selector(|| "edited".into())
            .flex_none()
            .px_1()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(edited_ago(now, at))
            .tooltip(move |window, cx| Tooltip::new(exact.clone()).build(window, cx))
            .into_any_element()
    }

    /// The star: filled while the page is a favourite; a click stars or unstars it.
    fn render_star(&self, page: PageId, cx: &mut Context<Self>) -> AnyElement {
        let on = self.editor.is_favourite(page);
        let icon = Icon::new(if on { IconName::StarFill } else { IconName::Star });
        let icon = if on {
            icon.text_color(gpui::rgb(0x00E8_B33D))
        } else {
            icon
        };
        crate::theme::button("favourite")
            .debug_selector(|| "favourite".into())
            .icon(icon)
            .ghost()
            .small()
            .tooltip(if on {
                "Remove from Favourites"
            } else {
                "Add to Favourites"
            })
            .on_click(cx.listener(move |shell, _: &ClickEvent, _, cx| shell.toggle_favourite(page, cx)))
            .into_any_element()
    }

    /// The `⋯` page menu: the editor's page menu for the top bar (E18-S04).
    fn render_more(page: PageId, cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        crate::theme::button("page-menu")
            .debug_selector(|| "page-menu".into())
            .icon(Icon::new(IconName::Ellipsis))
            .ghost()
            .small()
            .tooltip("More")
            .dropdown_menu(move |menu, _, cx| page_menu(menu, &shell, page, MenuPlace::TopBar, cx))
            .into_any_element()
    }

    /// Stars `page` if it isn't a favourite, unstars it if it is.
    pub(super) fn toggle_favourite(&mut self, page: PageId, cx: &mut Context<Self>) {
        let on = !self.editor.is_favourite(page);
        if let Err(err) = self.editor.set_favourite(page, on) {
            report("could not change Favourites", err);
        }
        cx.notify();
    }
}

/// The local time zone's offset from UTC at `at` (Unix milliseconds), in minutes, for the exact time
/// on hover. UTC if the time can't be read.
pub(super) fn local_offset_minutes(at: i64) -> i32 {
    Local
        .timestamp_millis_opt(at)
        .single()
        .map_or(0, |time| time.offset().fix().local_minus_utc() / 60)
}

#[cfg(test)]
mod tests;
