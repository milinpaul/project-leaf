//! The page cover picker, shown in a popover from the page header: a "Gallery" tab with "Remove" on the
//! right, then each section of built-in covers as a grid of thumbnails, four to a row (E06-S16). The page's
//! current cover is outlined. It tells the shell what was picked; it never changes a page itself.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, StyledImage as _, Window, div, img, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use leaf_editor::{BuiltinCover, CoverSection, PageCover};

use crate::covers;

/// A thumbnail's size, in pixels: wide like the cover band, four to a row.
const THUMB_WIDTH: f32 = 112.0;
const THUMB_HEIGHT: f32 = 60.0;

/// Thumbnails in a row.
const PER_ROW: f32 = 4.0;

/// The gap between thumbnails, in pixels.
const GAP: f32 = 8.0;

/// The outline around a thumbnail, in pixels: the accent colour on the current cover.
const RING: f32 = 2.0;

/// The tallest the list of covers gets before it scrolls, in pixels.
const MAX_LIST_HEIGHT: f32 = 360.0;

/// What the owner did in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPicked {
    /// This cover becomes the page's cover.
    Cover(BuiltinCover),
    /// "Remove": the page loses its cover.
    Remove,
    /// Esc: close without changing anything.
    Dismissed,
}

/// The picker's content, shown in a popover from the page header.
pub struct CoverPickerPanel {
    focus: FocusHandle,
    /// The page's cover when the picker opened, outlined in the grid; "Remove" shows only with one.
    current: Option<PageCover>,
}

impl EventEmitter<CoverPicked> for CoverPickerPanel {}

impl CoverPickerPanel {
    /// A picker over every built-in cover, for a page whose cover is `current`.
    pub fn new(current: Option<PageCover>, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
            current,
        }
    }

    fn render_choice(&self, cover: BuiltinCover, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (hover, accent, radius) = (theme.border, theme.primary, theme.radius);
        let selected = self.current == Some(PageCover::Builtin(cover));
        div()
            .id(SharedString::from(format!("cover-{}", cover.name())))
            .debug_selector(move || format!("cover-{}", cover.name()))
            .rounded(radius + px(RING))
            .border(px(RING))
            .border_color(gpui::transparent_black())
            .when(selected, |thumb| thumb.border_color(accent))
            .when(!selected, |thumb| thumb.hover(|style| style.border_color(hover)))
            .cursor_pointer()
            .tooltip(move |window, cx| Tooltip::new(cover.label()).build(window, cx))
            .child(
                img(covers::asset_path(cover))
                    .w(px(THUMB_WIDTH))
                    .h(px(THUMB_HEIGHT))
                    .object_fit(ObjectFit::Cover)
                    .rounded(radius),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _: &MouseDownEvent, _, cx| cx.emit(CoverPicked::Cover(cover))),
            )
            .into_any_element()
    }

    fn render_section(&self, section: CoverSection, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let choices: Vec<AnyElement> = section
            .covers()
            .map(|cover| self.render_choice(cover, cx))
            .collect();
        v_flex()
            .gap_2()
            .child(div().text_xs().text_color(muted).child(section.label()))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(px(GAP))
                    .w(px((THUMB_WIDTH + 2.0 * RING) * PER_ROW + GAP * (PER_ROW - 1.0)))
                    .children(choices),
            )
            .into_any_element()
    }

    /// The tab row: "Gallery", underlined as the open tab, and "Remove" on the right while the page has
    /// a cover.
    fn render_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (foreground, border, muted) = (theme.foreground, theme.border, theme.muted_foreground);
        let remove = self.current.is_some().then(|| {
            crate::theme::button("cover-picker-remove")
                .debug_selector(|| "cover-picker-remove".into())
                .ghost()
                .small()
                .text_color(muted)
                .label("Remove")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(CoverPicked::Remove)))
        });
        h_flex()
            .justify_between()
            .items_end()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .pb_1p5()
                    .border_b_2()
                    .border_color(foreground)
                    .text_sm()
                    .child("Gallery"),
            )
            .children(remove.map(|button| div().pb_1().child(button)))
            .into_any_element()
    }
}

impl Focusable for CoverPickerPanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CoverPickerPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections: Vec<AnyElement> = CoverSection::ALL
            .iter()
            .map(|&section| self.render_section(section, cx))
            .collect();
        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CoverPicked::Dismissed);
                    cx.stop_propagation();
                }
            }))
            .gap_3()
            .child(self.render_tabs(cx))
            .child(
                v_flex()
                    .id("cover-picker-list")
                    .max_h(px(MAX_LIST_HEIGHT))
                    .overflow_y_scroll()
                    .gap_4()
                    .children(sections),
            )
    }
}
