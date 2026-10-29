//! The quick switcher's popup (Cmd/Ctrl+P): a search field over a short list of pages, near the top
//! of the window like Notion's. It draws `leaf_editor::QuickSwitcher` and forwards typing, Up/Down,
//! Enter and clicks to it; choosing a page closes the popup and opens the page in the shell.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Render,
    ScrollStrategy, Styled, StyledText, Subscription, UniformListScrollHandle, WeakEntity, Window, div, px,
    uniform_list,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{QuickSwitcher, SwitcherResult, Vertical, WorkspaceEditor};

use crate::shell::Shell;

/// The popup's width and its distance from the top of the window, in pixels.
const WIDTH: f32 = 600.0;
const TOP: f32 = 96.0;

/// Height of a result row, in pixels; fixed so the list draws only the rows on screen.
const ROW_HEIGHT: f32 = 40.0;

/// Rows visible before the list scrolls.
const VISIBLE_ROWS: f32 = 9.0;

/// The popup's content: the switcher's state, its search field and the list's scroll position.
pub struct SwitcherPanel {
    switcher: QuickSwitcher,
    input: Entity<InputState>,
    shell: WeakEntity<Shell>,
    scroll: UniformListScrollHandle,
    _events: Subscription,
}

/// Opens the quick switcher over `editor`'s pages. Choosing one opens it in `shell`.
pub fn open(shell: WeakEntity<Shell>, editor: &WorkspaceEditor, window: &mut Window, cx: &mut gpui::App) {
    let switcher = match QuickSwitcher::open(editor.workspace(), editor.recent_pages()) {
        Ok(switcher) => switcher,
        Err(err) => return eprintln!("leaf: could not list pages for the switcher: {err}"),
    };
    let panel = cx.new(|cx| SwitcherPanel::new(switcher, shell, window, cx));
    let focus = panel.read(cx).input.read(cx).focus_handle(cx);
    window.open_dialog(cx, move |dialog, _, _| {
        let chooser = panel.clone();
        dialog
            .p_0()
            .w(px(WIDTH))
            .margin_top(px(TOP))
            .close_button(false)
            // Enter is the dialog's "OK": open the highlighted page. The panel closes the dialog itself,
            // and only when there is a page to open, so Enter with no results leaves it open.
            .on_ok(move |_, window, cx| {
                chooser.update(cx, |panel, cx| panel.choose(window, cx));
                false
            })
            .child(panel.clone())
    });
    window.focus(&focus, cx);
}

impl SwitcherPanel {
    fn new(
        switcher: QuickSwitcher,
        shell: WeakEntity<Shell>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search pages by title…"));
        let events = cx.subscribe_in(&input, window, |panel, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                panel.switcher.set_query(&query);
                panel.scroll.scroll_to_item(0, ScrollStrategy::Top);
                cx.notify();
            }
        });
        Self {
            switcher,
            input,
            shell,
            scroll: UniformListScrollHandle::new(),
            _events: events,
        }
    }

    /// Up and Down move the highlight (the field would otherwise move its caret) and keep it in view.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let direction = match event.keystroke.key.as_str() {
            "up" => Vertical::Up,
            "down" => Vertical::Down,
            _ => return,
        };
        cx.stop_propagation();
        self.switcher.move_selection(direction);
        if let Some(selected) = self.switcher.selected() {
            self.scroll.scroll_to_item(selected, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Opens the highlighted page and closes the popup. With no results, Enter does nothing.
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.switcher.chosen() else {
            return;
        };
        window.close_dialog(cx);
        let _ = self.shell.update(cx, |shell, cx| shell.open(page, window, cx));
    }

    fn render_row(&self, ix: usize, result: &SwitcherResult, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme();
        let (selected_bg, muted, text) = (theme.sidebar_accent, theme.muted_foreground, theme.foreground);
        let selected = self.switcher.selected() == Some(ix);
        let bold = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            color: Some(text),
            ..HighlightStyle::default()
        };
        let title = StyledText::new(result.label.clone())
            .with_highlights(result.highlights.iter().map(|range| (range.clone(), bold)));
        let icon = match &result.icon {
            Some(emoji) => div().child(emoji.clone()).into_any_element(),
            None => Icon::new(IconName::FileText)
                .small()
                .text_color(muted)
                .into_any_element(),
        };
        h_flex()
            .id(("switcher-row", ix))
            .debug_selector(|| format!("switcher-row-{ix}"))
            .w_full()
            .h(px(ROW_HEIGHT))
            .px_3()
            .gap_3()
            .rounded(theme.radius)
            .when(selected, |row| row.bg(selected_bg))
            .child(div().flex_none().w(px(20.0)).flex().justify_center().child(icon))
            .child(
                div()
                    .flex_none()
                    .max_w(px(300.0))
                    .truncate()
                    .text_sm()
                    .child(title),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(muted)
                    .child(result.path.clone()),
            )
            .on_mouse_move(cx.listener(move |panel, _, _, cx| {
                if panel.switcher.selected() != Some(ix) {
                    panel.switcher.select(ix);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |panel, _: &MouseDownEvent, window, cx| {
                    panel.switcher.select(ix);
                    panel.choose(window, cx);
                }),
            )
            .into_any_element()
    }
}

impl Focusable for SwitcherPanel {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for SwitcherPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let count = self.switcher.results().len();
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
            );
        let body = if count == 0 {
            div()
                .h(px(ROW_HEIGHT * 2.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child("No pages match")
                .into_any_element()
        } else {
            let height = ROW_HEIGHT * VISIBLE_ROWS.min(f32::from(u16::try_from(count).unwrap_or(u16::MAX)));
            uniform_list(
                "switcher-results",
                count,
                cx.processor(|panel, range: std::ops::Range<usize>, _, cx| {
                    let results = panel.switcher.results().to_vec();
                    range
                        .filter_map(|ix| results.get(ix).map(|result| panel.render_row(ix, result, cx)))
                        .collect::<Vec<_>>()
                }),
            )
            .track_scroll(&self.scroll)
            .h(px(height))
            .px_2()
            .into_any_element()
        };
        // With nothing typed the list leads with recently opened pages, then every other one.
        let label = "Pages";
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
            .id("switcher")
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

/// A keyboard hint at the popup's foot: key chips, then what they do.
fn hint<const N: usize>(keys: [gpui::AnyElement; N], does: &'static str) -> gpui::AnyElement {
    h_flex()
        .gap_1()
        .children(keys)
        .child(div().ml_0p5().child(does))
        .into_any_element()
}

/// A key chip around `content`.
fn key_chip(content: gpui::AnyElement, cx: &gpui::App) -> gpui::AnyElement {
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

fn key_icon(icon: IconName, cx: &gpui::App) -> gpui::AnyElement {
    key_chip(Icon::new(icon).xsmall().into_any_element(), cx)
}

fn key_text(text: &'static str, cx: &gpui::App) -> gpui::AnyElement {
    key_chip(div().text_size(px(10.0)).child(text).into_any_element(), cx)
}

#[cfg(test)]
mod tests;
