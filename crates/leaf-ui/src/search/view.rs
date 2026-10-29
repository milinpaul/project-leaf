//! Drawing the search popup: the field, then the results grouped by page (a page row with icon, title
//! and breadcrumb, then its matching lines indented beneath with the matched words highlighted), a
//! state line when there is nothing to list, and key hints at the foot.

use std::ops::Range;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, StyledText,
    Window, div, px,
};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{ExcerptRow, PageRow, SearchRow};

use super::SearchPanel;

/// Height of a page row and of a matching line beneath it, in pixels.
const PAGE_ROW: f32 = 40.0;
const EXCERPT_ROW: f32 = 30.0;

/// The results' height before they scroll.
const LIST_HEIGHT: f32 = 380.0;

/// Where a matching line's text starts, lined up with the page title above it.
const EXCERPT_INDENT: f32 = 44.0;

impl Render for SearchPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let field = h_flex()
            .h(px(52.0))
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(border)
            .child(Icon::new(IconName::Search).text_color(muted))
            .child(
                div().flex_1().child(
                    Input::new(&self.input)
                        .appearance(false)
                        .focus_bordered(false)
                        .px_0(),
                ),
            )
            .when(self.bar.searching(), |field| {
                field.child(div().text_xs().text_color(muted).child("Searching…"))
            });
        let blank = self.bar.query().trim().is_empty();
        let label = if blank { "Recent pages" } else { "Results" };
        let rows: Vec<SearchRow> = self.bar.rows().to_vec();
        let body = if rows.is_empty() {
            let message: SharedString = if blank {
                "No pages yet".into()
            } else if self.bar.searching() {
                "Searching…".into()
            } else {
                format!("No results for “{}”", self.bar.query().trim()).into()
            };
            div()
                .debug_selector(|| "search-empty".into())
                .h(px(PAGE_ROW * 2.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(message)
                .into_any_element()
        } else {
            v_flex()
                .id("search-results")
                .max_h(px(LIST_HEIGHT))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .px_2()
                .children(
                    rows.iter()
                        .enumerate()
                        .map(|(ix, row)| self.render_row(ix, row, cx)),
                )
                .into_any_element()
        };
        let hints = h_flex()
            .h(px(36.0))
            .px_4()
            .gap_4()
            .border_t_1()
            .border_color(border)
            .text_xs()
            .text_color(muted)
            .child(hint(
                [key_icon(IconName::ArrowUp, cx), key_icon(IconName::ArrowDown, cx)],
                "to move",
            ))
            .child(hint([key_text("Enter", cx)], "to open"))
            .child(hint([key_text("Esc", cx)], "to close"));
        v_flex()
            .id("search")
            .capture_key_down(cx.listener(Self::on_key_down))
            .child(field)
            .child(
                div()
                    .px_5()
                    .pt_3()
                    .pb_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(label),
            )
            .child(div().pb_2().child(body))
            .child(hints)
    }
}

impl SearchPanel {
    fn render_row(&self, ix: usize, row: &SearchRow, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.bar.selected() == Some(ix);
        let selected_bg = cx.theme().sidebar_accent;
        let radius = cx.theme().radius;
        let (content, height) = match row {
            SearchRow::Page(page) => (page_content(page, cx), PAGE_ROW),
            SearchRow::Excerpt(excerpt) => (excerpt_content(excerpt, cx), EXCERPT_ROW),
        };
        h_flex()
            .id(("search-row", ix))
            .debug_selector(move || format!("search-row-{ix}"))
            .w_full()
            .h(px(height))
            .flex_none()
            .px_3()
            .gap_3()
            .rounded(radius)
            .when(selected, |row| row.bg(selected_bg))
            .child(content)
            .on_mouse_move(cx.listener(move |panel, _, _, cx| {
                if panel.bar.selected() != Some(ix) {
                    panel.bar.select(ix);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |panel, _: &MouseDownEvent, window, cx| {
                    panel.bar.select(ix);
                    panel.choose(window, cx);
                }),
            )
            .into_any_element()
    }
}

/// A page: icon, title with the matched letters bold, and where it lives.
fn page_content(page: &PageRow, cx: &Context<SearchPanel>) -> AnyElement {
    let theme = cx.theme();
    let (muted, text) = (theme.muted_foreground, theme.foreground);
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        color: Some(text),
        ..HighlightStyle::default()
    };
    let icon = match &page.icon {
        Some(emoji) => div().child(emoji.clone()).into_any_element(),
        None => Icon::new(IconName::FileText)
            .small()
            .text_color(muted)
            .into_any_element(),
    };
    h_flex()
        .flex_1()
        .min_w_0()
        .gap_3()
        .child(div().flex_none().w(px(20.0)).flex().justify_center().child(icon))
        .child(
            div()
                .flex_none()
                .max_w(px(320.0))
                .truncate()
                .text_sm()
                .child(styled(&page.label, &page.highlights, bold)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(muted)
                .child(page.path.clone()),
        )
        .into_any_element()
}

/// A matching line under its page: the excerpt, the matched words bold on a soft tint.
fn excerpt_content(excerpt: &ExcerptRow, cx: &Context<SearchPanel>) -> AnyElement {
    let theme = cx.theme();
    let (muted, text, tint) = (theme.muted_foreground, theme.foreground, theme.primary);
    let matched = HighlightStyle {
        font_weight: Some(FontWeight::SEMIBOLD),
        color: Some(text),
        background_color: Some(tint.opacity(0.16)),
        ..HighlightStyle::default()
    };
    div()
        .flex_1()
        .min_w_0()
        .pl(px(EXCERPT_INDENT - 12.0))
        .truncate()
        .text_xs()
        .text_color(muted)
        .child(styled(&excerpt.text, &excerpt.highlights, matched))
        .into_any_element()
}

fn styled(text: &str, ranges: &[Range<usize>], style: HighlightStyle) -> StyledText {
    StyledText::new(text.to_owned()).with_highlights(ranges.iter().map(|range| (range.clone(), style)))
}

/// A keyboard hint at the popup's foot: key chips, then what they do.
fn hint<const N: usize>(keys: [AnyElement; N], does: &'static str) -> AnyElement {
    h_flex()
        .gap_1()
        .children(keys)
        .child(div().ml_0p5().child(does))
        .into_any_element()
}

/// A key chip around `content`.
fn key_chip(content: AnyElement, cx: &gpui::App) -> AnyElement {
    let theme = cx.theme();
    div()
        .h(px(18.0))
        .min_w(px(18.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .border_1()
        .border_color(theme.border)
        .bg(theme.secondary)
        .child(content)
        .into_any_element()
}

fn key_icon(icon: IconName, cx: &gpui::App) -> AnyElement {
    key_chip(Icon::new(icon).xsmall().into_any_element(), cx)
}

fn key_text(text: &'static str, cx: &gpui::App) -> AnyElement {
    key_chip(div().text_size(px(10.0)).child(text).into_any_element(), cx)
}
