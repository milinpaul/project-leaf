//! The floating formatting bar over selected text (E07-S13, ADR 0067): "Turn into", then Bold, Italic,
//! Strikethrough, Code and Link. What it offers, and which marks are on, is `PageEditor::format_bar`;
//! this module places and draws it and runs a click the way the keyboard would (`toggle_mark`, the link
//! popover, `BlockAction::TurnInto`), then gives the page the keyboard back with the selection kept.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, Keystroke, MouseButton, MouseDownEvent,
    ParentElement as _, Pixels, Point, Styled as _, Window, anchored, deferred, div, point, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::kbd::Kbd;
use gpui_component::{ActiveTheme as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{FormatBar, KindChoice, MarkButton};
use leaf_model::{BlockKind, Caret, MarkKind};

use super::{Shell, report};

/// The bar's height, in pixels, to place it above the selection.
const BAR_HEIGHT: f32 = 36.0;

/// The gap between the bar and the selected line, in pixels.
const GAP: f32 = 6.0;

/// The bar's state between frames.
#[derive(Default)]
pub(super) struct FormatBarState {
    /// The selection whose "Turn into" list is open. The list is drawn in the bar's own overlay, not
    /// as a menu that takes the keyboard, so the page keeps its selection; selecting something else
    /// closes it.
    kinds_open: Option<(Caret, Caret)>,
}

impl Shell {
    /// The bar for the current selection, placed by where the selection was drawn last frame, or
    /// `None` when there is no text selected or something else is in the way (a drag in progress, the
    /// link popover, a menu). Call before the painted layout is cleared for the next frame.
    pub(super) fn render_format_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let has_keyboard = self.page_focus.is_focused(window);
        let busy = self.blocks.drag.is_some()
            || self.blocks.link.is_some()
            || self.blocks.language.is_some()
            || self.blocks.tones.is_some()
            || cx.has_active_drag();
        if !has_keyboard || busy {
            return None;
        }
        let page = self.page()?;
        if matches!(page.slash_menu(), Ok(Some(_))) || matches!(page.block_menu(), Ok(Some(_))) {
            return None;
        }
        let bar = match page.format_bar() {
            Ok(bar) => bar?,
            Err(err) => {
                report("could not read the selection's formatting", err);
                return None;
            }
        };
        let at = self.format_bar_position(&bar)?;
        let open = self.blocks.format_bar.kinds_open == Some((bar.start, bar.end));
        if !open {
            self.blocks.format_bar.kinds_open = None;
        }
        let content = Self::render_format_bar_content(&bar, open, cx);
        let list = open.then(|| Self::render_kind_list(&bar.kinds, cx));
        Some(
            deferred(
                anchored()
                    .position(at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(v_flex().gap_1().child(content).children(list)),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// Just above where the selection starts; below the line where it ends when that would go past
    /// the top of the page's view.
    fn format_bar_position(&self, bar: &FormatBar) -> Option<Point<Pixels>> {
        let painted = &self.blocks.painted;
        let top = self.blocks.list.state.viewport_bounds().top();
        let above = painted
            .caret_bounds(bar.start.block, bar.start.at)
            .map(|(origin, _)| point(origin.x - px(4.0), origin.y - px(BAR_HEIGHT + GAP)))
            .filter(|at| at.y >= top);
        above.or_else(|| {
            painted
                .caret_bounds(bar.end.block, bar.end.at)
                .map(|(origin, height)| point(origin.x - px(4.0), origin.y + height + px(GAP)))
        })
    }

    fn render_format_bar_content(bar: &FormatBar, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (background, border, radius) = (theme.popover, theme.border, theme.radius);
        let divider = || div().w(px(1.0)).h(px(20.0)).mx_0p5().bg(border);
        let turn_into = (!bar.kinds.is_empty()).then(|| Self::render_turn_into(bar, open, cx));
        let marks: Vec<AnyElement> = bar
            .marks
            .iter()
            .map(|mark| Self::render_mark(*mark, cx))
            .collect();
        let has_both = turn_into.is_some() && !marks.is_empty();
        h_flex()
            .id("format-bar")
            .debug_selector(|| "format-bar".into())
            .occlude()
            .h(px(BAR_HEIGHT))
            .px_1()
            .gap_0p5()
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(background)
            .shadow_md()
            .children(turn_into)
            .children(has_both.then(divider))
            .children(marks)
            .into_any_element()
    }

    /// "Turn into ▾", naming the selected blocks' kind: opens or closes the list of kinds.
    fn render_turn_into(bar: &FormatBar, open: bool, cx: &mut Context<Self>) -> AnyElement {
        let selection = (bar.start, bar.end);
        crate::theme::button("format-turn-into")
            .debug_selector(|| "format-turn-into".into())
            .ghost()
            .small()
            .label(bar.kind_label)
            .dropdown_caret(true)
            .selected(open)
            .on_click(cx.listener(move |shell, _, _, cx| {
                let state = &mut shell.blocks.format_bar.kinds_open;
                *state = if state.is_some() { None } else { Some(selection) };
                cx.notify();
            }))
            .into_any_element()
    }

    /// The kinds "Turn into" offers, the selected blocks' kind ticked; choosing one changes every block
    /// the selection touches.
    fn render_kind_list(kinds: &[KindChoice], cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (background, border, hover, radius) = (theme.popover, theme.border, theme.accent, theme.radius);
        let rows: Vec<AnyElement> = kinds
            .iter()
            .enumerate()
            .map(|(ix, choice)| {
                let kind: BlockKind = choice.kind.clone();
                h_flex()
                    .id(("format-kind", ix))
                    .debug_selector(move || format!("format-kind-{ix}"))
                    .h(px(30.0))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .text_sm()
                    .cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .child(div().flex_1().child(choice.label))
                    .when(choice.current, |row| {
                        row.child(Icon::new(IconName::Check).small())
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            shell.blocks.format_bar.kinds_open = None;
                            shell.turn_focused_into(kind.clone(), window, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .id("format-kinds")
            .debug_selector(|| "format-kinds".into())
            .occlude()
            .w(px(200.0))
            .p_1()
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(background)
            .shadow_md()
            .children(rows)
            .into_any_element()
    }

    /// One mark's button: on while the whole selection has the mark; a click toggles it, or for Link
    /// opens the link popover.
    fn render_mark(mark: MarkButton, cx: &mut Context<Self>) -> AnyElement {
        let icon = match mark.kind {
            MarkKind::Bold => IconName::Bold,
            MarkKind::Italic => IconName::Italic,
            MarkKind::Strike => IconName::Strikethrough,
            MarkKind::Code => IconName::Code,
            MarkKind::Link => IconName::Link,
        };
        // The same keys `page_keys` binds.
        let keys = match mark.kind {
            MarkKind::Bold => "secondary-b",
            MarkKind::Italic => "secondary-i",
            MarkKind::Strike => "secondary-shift-s",
            MarkKind::Code => "secondary-e",
            MarkKind::Link => "secondary-k",
        };
        let tooltip = Keystroke::parse(keys).ok().map_or_else(
            || mark.label.to_owned(),
            |keys| format!("{}  {}", mark.label, Kbd::format(&keys)),
        );
        let id = format!("format-{}", mark.label.to_lowercase());
        crate::theme::button(id.clone())
            .debug_selector(move || id.clone())
            .ghost()
            .small()
            .icon(Icon::new(icon))
            .selected(mark.active)
            .tooltip(tooltip)
            .on_click(cx.listener(move |shell, _, window, cx| {
                if mark.kind == MarkKind::Link {
                    shell.focus_page(window, cx);
                    shell.open_link_popover(window, cx);
                    return;
                }
                if let Some(Err(err)) = shell.page().map(|page| page.toggle_mark(mark.kind)) {
                    report("could not format", err);
                }
                shell.focus_page(window, cx);
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
