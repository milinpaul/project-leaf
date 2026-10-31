//! The one-time question about web bookmark previews (E15-S06, ADR-080): the first time a web bookmark
//! wants a preview before the owner has decided, a bar at the bottom of the page asks whether Leaf may
//! go online for it, and says what that means. "Allow" fetches the waiting previews; "Not now" keeps
//! Leaf offline and is never asked again. Settings › Privacy changes the answer later. Tested with the
//! fetching itself, in `preview_fetch/tests.rs`.

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div, px,
};
use gpui_component::button::ButtonVariants as _;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;

use super::Shell;

/// What the bar says.
pub(crate) const QUESTION: &str = "Fetch previews for web bookmarks?";
/// What going online means, in one line; Settings › Privacy says the same.
pub(crate) const EXPLANATION: &str = "Leaf visits the bookmark's address from this computer to read the page's title, \
     description and pictures, so the site sees your IP address. Public websites only; nothing else is sent.";

impl Shell {
    /// The owner's answer: saved at once, then the waiting previews are fetched, or dropped.
    pub(super) fn answer_previews(&mut self, allow: bool, cx: &mut Context<Self>) {
        crate::settings::update(cx, |settings| settings.set_web_previews(allow));
        crate::settings::save(cx);
        self.blocks.asking_previews = false;
        self.fetch_previews_if_asked(cx);
        cx.notify();
    }

    /// The bar, while the question is open.
    pub(super) fn render_previews_question(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.blocks.asking_previews {
            return None;
        }
        let theme = cx.theme();
        let (bg, border, muted, radius) = (theme.popover, theme.border, theme.muted_foreground, theme.radius);
        Some(
            div()
                .absolute()
                .bottom_6()
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    h_flex()
                        .id("previews-question")
                        .debug_selector(|| "previews-question".into())
                        .occlude()
                        .max_w(px(640.0))
                        .mx_4()
                        .gap_3()
                        .items_start()
                        .p_3()
                        .rounded(radius)
                        .border_1()
                        .border_color(border)
                        .bg(bg)
                        .shadow_lg()
                        .child(Icon::new(IconName::Bookmark).text_color(muted))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(QUESTION),
                                )
                                .child(div().text_xs().text_color(muted).child(EXPLANATION)),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .flex_none()
                                .child(
                                    div().debug_selector(|| "previews-not-now".into()).child(
                                        crate::theme::button("previews-not-now")
                                            .ghost()
                                            .small()
                                            .label("Not now")
                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                shell.answer_previews(false, cx);
                                            })),
                                    ),
                                )
                                .child(
                                    div().debug_selector(|| "previews-allow".into()).child(
                                        crate::theme::button("previews-allow")
                                            .primary()
                                            .small()
                                            .label("Allow")
                                            .on_click(cx.listener(|shell, _, _, cx| {
                                                shell.answer_previews(true, cx);
                                            })),
                                    ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
