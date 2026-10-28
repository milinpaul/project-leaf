//! A window that only explains why the workspace could not be opened.

use gpui::{IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::{ActiveTheme as _, v_flex};

/// Shown instead of the editor when the workspace can't be opened (already open in another Leaf
/// window, damaged, unreadable), so the user sees why rather than an empty or vanished window.
pub struct Notice {
    title: String,
    detail: String,
}

impl Notice {
    /// A notice with a short headline and the underlying reason.
    #[must_use]
    pub fn new(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
        }
    }
}

impl Render for Notice {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .p_8()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_xl().child(self.title.clone()))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.detail.clone()),
            )
    }
}
