//! The page cover picker: a thumbnail of every built-in cover, shown in a popover from the page header.
//! It tells the shell what was picked; it never changes a page itself.

use gpui::{
    AnyElement, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ObjectFit, ParentElement, Render, SharedString, Styled,
    StyledImage as _, Window, div, img, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use leaf_editor::BuiltinCover;

use crate::covers;

/// A thumbnail's size, in pixels: the cover's 4:1 shape, small enough for three in a row.
const THUMB_WIDTH: f32 = 128.0;
const THUMB_HEIGHT: f32 = 32.0;

/// What the owner did in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPicked {
    /// This cover becomes the page's cover.
    Cover(BuiltinCover),
    /// Esc: close without changing anything.
    Dismissed,
}

/// The picker's content, shown in a popover from the page header.
pub struct CoverPickerPanel {
    focus: FocusHandle,
}

impl EventEmitter<CoverPicked> for CoverPickerPanel {}

impl CoverPickerPanel {
    /// A picker over every built-in cover.
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }

    fn render_choice(cover: BuiltinCover, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (hover, radius) = (theme.secondary_hover, theme.radius);
        v_flex()
            .id(SharedString::from(format!("cover-{}", cover.name())))
            .debug_selector(move || format!("cover-{}", cover.name()))
            .p_1()
            .gap_1()
            .rounded(radius)
            .hover(|style| style.bg(hover))
            .child(
                img(covers::asset_path(cover))
                    .w(px(THUMB_WIDTH))
                    .h(px(THUMB_HEIGHT))
                    .object_fit(ObjectFit::Cover)
                    .rounded(radius),
            )
            .child(div().text_xs().child(cover.label()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_, _: &MouseDownEvent, _, cx| cx.emit(CoverPicked::Cover(cover))),
            )
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
        let muted = cx.theme().muted_foreground;
        let choices: Vec<AnyElement> = BuiltinCover::ALL
            .iter()
            .map(|&cover| Self::render_choice(cover, cx))
            .collect();
        v_flex()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(CoverPicked::Dismissed);
                    cx.stop_propagation();
                }
            }))
            .gap_2()
            .child(div().text_xs().text_color(muted).child("Covers"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .max_w(px((THUMB_WIDTH + 12.0) * 3.0))
                    .children(choices),
            )
    }
}
