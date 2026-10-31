//! Callouts (E21-S02, ADR 0070): a rounded box in its tone's soft colour with the tone's icon on the
//! left, and a small picker of the five tones that opens from the icon. The text inside is drawn and
//! edited like any other block's; the tone changes through `PageEditor::set_callout_tone`.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    Pixels, Point, SharedString, Styled, anchored, deferred, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::BlockId;
use leaf_editor::BlockRow;
use leaf_model::CalloutTone;

use super::block_row::indent_of;
use super::{Shell, report};
use crate::theme;

/// Room inside the box around the text, in pixels: across, and above and below.
pub(super) const PAD_X: f32 = 12.0;
pub(super) const PAD_Y: f32 = 8.0;

/// The tone picker while it is open: which callout, and where it floats.
pub(super) struct OpenTones {
    pub(super) block: BlockId,
    at: Point<Pixels>,
}

/// A tone's icon. A tone from a newer version shows the note's bulb.
fn icon_of(tone: &CalloutTone) -> IconName {
    match tone {
        CalloutTone::Info => IconName::Info,
        CalloutTone::Success => IconName::CircleCheck,
        CalloutTone::Warning => IconName::TriangleAlert,
        CalloutTone::Error => IconName::CircleX,
        CalloutTone::Note | CalloutTone::Other(_) => IconName::Lightbulb,
    }
}

/// A tone's name in the picker.
fn label_of(tone: &CalloutTone) -> &'static str {
    match tone {
        CalloutTone::Info => "Info",
        CalloutTone::Success => "Success",
        CalloutTone::Warning => "Warning",
        CalloutTone::Error => "Error",
        CalloutTone::Note | CalloutTone::Other(_) => "Note",
    }
}

impl Shell {
    /// A callout row: the box, its icon (a button that opens the tone picker) and `body`, the text.
    pub(super) fn render_callout(
        row: &BlockRow,
        tone: &CalloutTone,
        body: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = row.id;
        let dark = cx.theme().mode.is_dark();
        let (background, accent) = theme::callout_colours(tone, dark);
        let radius = cx.theme().radius;
        let icon = div()
            .id(SharedString::from(format!("callout-icon-{id}")))
            .debug_selector(move || format!("callout-icon-{id}"))
            .flex_none()
            .mt(px(3.0))
            .rounded(radius)
            .cursor_pointer()
            .hover(|style| style.bg(accent.opacity(0.12)))
            .child(Icon::new(icon_of(tone)).text_color(accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    // Not also a click that places the caret in the text.
                    cx.stop_propagation();
                    let at = point(event.position.x - px(8.0), event.position.y + px(14.0));
                    shell.blocks.tones = Some(OpenTones { block: id, at });
                    cx.notify();
                }),
            );
        h_flex()
            .w_full()
            .pl(indent_of(row))
            .child(
                h_flex()
                    .debug_selector(move || format!("callout-{id}"))
                    .flex_1()
                    .min_w_0()
                    .items_start()
                    .gap_2()
                    .px(px(PAD_X))
                    .py(px(PAD_Y))
                    .rounded(radius)
                    .bg(background)
                    .child(icon)
                    .child(div().flex_1().min_w_0().child(body)),
            )
            .into_any_element()
    }

    /// The tone picker, floating under the icon that opened it: the five tones, the callout's own
    /// ticked. Choosing one changes the tone; a click anywhere else closes it.
    pub(super) fn render_tone_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.blocks.tones.as_ref()?;
        let (block, at) = (open.block, open.at);
        // The callout may have gone (deleted, or turned into something else): nothing to pick for.
        let Some(Ok(leaf_model::BlockKind::Callout { tone: current })) =
            self.page().map(|page| page.doc().kind(block))
        else {
            self.blocks.tones = None;
            return None;
        };
        let theme = cx.theme();
        let (popover, border, hover, radius, dark) = (
            theme.popover,
            theme.border,
            theme.accent,
            theme.radius,
            theme.mode.is_dark(),
        );
        let rows: Vec<AnyElement> = CalloutTone::ALL
            .into_iter()
            .map(|tone| {
                let (_, accent) = theme::callout_colours(&tone, dark);
                let chosen = tone == current;
                let name = tone.as_str().to_owned();
                h_flex()
                    .id(SharedString::from(format!("callout-tone-{name}")))
                    .debug_selector(move || format!("callout-tone-{name}"))
                    .h(px(30.0))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .text_sm()
                    .cursor_pointer()
                    .hover(|style| style.bg(hover))
                    .child(Icon::new(icon_of(&tone)).small().text_color(accent))
                    .child(div().flex_1().child(label_of(&tone)))
                    .when(chosen, |row| row.child(Icon::new(IconName::Check).small()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            shell.blocks.tones = None;
                            if let Some(Err(err)) = shell
                                .page()
                                .map(|page| page.set_callout_tone(block, tone.clone()))
                            {
                                report("could not change the callout", err);
                            }
                            shell.focus_page(window, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        Some(
            deferred(
                anchored().position(at).snap_to_window_with_margin(px(8.0)).child(
                    v_flex()
                        .id("callout-tones")
                        .debug_selector(|| "callout-tones".into())
                        .occlude()
                        .w(px(180.0))
                        .p_1()
                        .rounded(radius)
                        .border_1()
                        .border_color(border)
                        .bg(popover)
                        .shadow_md()
                        .on_mouse_down_out(cx.listener(|shell, _: &MouseDownEvent, _, cx| {
                            shell.blocks.tones = None;
                            cx.notify();
                        }))
                        .children(rows),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests;
