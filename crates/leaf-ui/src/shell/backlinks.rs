//! The backlinks line under the page title (E10-S07, ADR-071): "3 backlinks" when other pages link here,
//! nothing otherwise. A click opens the list in place: each linking page's icon and title, and the words
//! around its link with the link itself in the link colour. Choosing one opens that page at the block,
//! as a search result does.
//!
//! The index answers which blocks link here (`leaf_search::Searcher::backlinks`, well under a millisecond);
//! it is asked again when another page opens and at most once a second while one stays open, so the list
//! catches up with saves on its own. Titles and icons come from the workspace on every draw.

use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::PageId;
use leaf_editor::{BacklinkRow, BacklinkSource, Destination, backlinks_label};

use super::Shell;

/// How long a page's backlinks are kept before the index is asked again.
const REFRESH: Duration = Duration::from_secs(1);

/// The open page's backlinks between frames.
#[derive(Default)]
pub(super) struct BacklinksState {
    /// The page they belong to.
    page: Option<PageId>,
    /// What the index found for it.
    pub(super) found: Vec<BacklinkSource>,
    /// When the index was last asked; `None` before the first time.
    asked: Option<Instant>,
    /// Whether the list is open under the line.
    pub(super) open: bool,
}

impl Shell {
    /// The line under the title and, when open, the list; `None` when nothing links to the open page.
    pub(super) fn render_backlinks(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let page = self.editor.selected()?;
        self.refresh_backlinks(page, cx);
        let rows = self.editor.backlinks(page, self.blocks.backlinks.found.clone());
        let label = backlinks_label(rows.len())?;
        let open = self.blocks.backlinks.open;
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let line = h_flex()
            .id("backlinks-toggle")
            .debug_selector(|| "backlinks-toggle".into())
            .gap_1()
            .text_sm()
            .text_color(muted)
            .cursor_pointer()
            .hover(|style| style.text_color(theme.foreground))
            .child(Icon::new(IconName::Link).xsmall())
            .child(label)
            .child(
                Icon::new(if open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .xsmall(),
            )
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.blocks.backlinks.open = !shell.blocks.backlinks.open;
                cx.notify();
            }));
        let list = open.then(|| Self::render_backlink_list(&rows, cx));
        Some(
            v_flex()
                .debug_selector(|| "backlinks".into())
                .gap_1()
                .pb_2()
                .child(line)
                .children(list)
                .into_any_element(),
        )
    }

    /// Opens the backlinks list, as a click on its line does (for the screenshot tool).
    pub fn show_backlinks(&mut self, cx: &mut Context<Self>) {
        self.blocks.backlinks.open = true;
        cx.notify();
    }

    fn render_backlink_list(rows: &[BacklinkRow], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, link, hover, radius) = (theme.muted_foreground, theme.link, theme.accent, theme.radius);
        v_flex()
            .id("backlinks-list")
            .debug_selector(|| "backlinks-list".into())
            .gap_0p5()
            .children(rows.iter().enumerate().map(|(ix, row)| {
                let destination = Destination {
                    page: row.page,
                    block: Some(row.block),
                };
                v_flex()
                    .id(("backlink", ix))
                    .debug_selector(move || format!("backlink-{ix}"))
                    .px_2()
                    .py_1()
                    .rounded(radius)
                    .cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .child(
                        h_flex()
                            .gap_1p5()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .children(row.icon.clone())
                            .when(row.icon.is_none(), |title| {
                                title.child(Icon::new(IconName::FileText).xsmall().text_color(muted))
                            })
                            .child(row.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(row.before.clone())
                            .child(div().child(row.link.clone()).text_color(link))
                            .child(row.after.clone())
                            .flex()
                            .flex_wrap(),
                    )
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |shell, _, window: &mut Window, cx| {
                        shell.blocks.backlinks.open = false;
                        shell.open_at(destination, window, cx);
                    }))
            }))
            .into_any_element()
    }

    /// Asks the index again when another page is open or a second has passed. Without a workspace folder
    /// (an in-memory workspace in tests) there is no index, and whatever was set stays.
    fn refresh_backlinks(&mut self, page: PageId, cx: &mut Context<Self>) {
        let state = &mut self.blocks.backlinks;
        let other_page = state.page != Some(page);
        if other_page {
            state.page = Some(page);
            state.open = false;
        }
        let fresh = state.asked.is_some_and(|asked| asked.elapsed() < REFRESH);
        if !other_page && fresh {
            return;
        }
        let Some(root) = self.storage.as_ref().map(|storage| storage.root().to_owned()) else {
            return;
        };
        state.asked = Some(Instant::now());
        match leaf_search::Searcher::open(&root).and_then(|searcher| searcher.backlinks(page)) {
            Ok(found) => {
                state.found = found
                    .into_iter()
                    .map(|link| BacklinkSource {
                        page: link.page,
                        block: link.block,
                        before: link.before,
                        after: link.after,
                    })
                    .collect();
            }
            // No index yet (it is being built) reads as no backlinks for now.
            Err(_) => state.found.clear(),
        }
        // Once more in a moment after a page opens, so it catches up with the indexer if a link to it
        // was saved just before; later draws (typing, the caret's blink) keep it current after that.
        if other_page {
            cx.spawn(async move |shell, cx| {
                cx.background_executor()
                    .timer(REFRESH + Duration::from_millis(100))
                    .await;
                let _ = shell.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests;
