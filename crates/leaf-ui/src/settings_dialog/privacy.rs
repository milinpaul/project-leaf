//! Settings › Privacy (E15-S06, ADR-080): whether Leaf may go online to fetch web bookmark previews. Off
//! until the owner turns it on here or answers "Allow" the first time a bookmark wants a preview; the
//! answer is the device's (`settings.json`), not the workspace's.

use gpui::{AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

use super::{SettingsPanel, card};
use crate::settings;

impl SettingsPanel {
    /// The Privacy section: one switch, with what turning it on sends.
    pub(super) fn privacy(cx: &mut Context<Self>) -> AnyElement {
        let on = settings::get(cx).web_previews_allowed();
        let muted = cx.theme().muted_foreground;
        card(cx)
            .gap_3()
            .child(
                h_flex()
                    .gap_4()
                    .justify_between()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Fetch previews for web bookmarks"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(crate::shell::PREVIEWS_EXPLANATION),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .debug_selector(|| "privacy-previews".into())
                            .child(Switch::new("privacy-previews").checked(on).on_click(cx.listener(
                                |_, checked: &bool, _, cx| {
                                    settings::update(cx, |s| s.set_web_previews(*checked));
                                    settings::save(cx);
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .child(
                div().text_xs().text_color(muted).child(
                    "When it's off, Leaf never goes online: web bookmarks show their address and site.",
                ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, MouseButton, TestAppContext};

    use super::super::{Section, SettingsPanel, WorkspaceInfo};
    use crate::settings;

    #[gpui::test]
    fn the_privacy_switch_turns_previews_on_and_off(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(None, cx));
        let (_panel, cx) = cx.add_window_view(|_, cx| SettingsPanel {
            section: Section::Privacy,
            workspace: WorkspaceInfo::default(),
            focus: cx.focus_handle(),
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            !cx.update(|_, cx| settings::get(cx).web_previews_allowed()),
            "off by default"
        );
        for expected in [true, false] {
            let at = cx.debug_bounds("privacy-previews").unwrap().center();
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let now = cx.update(|_, cx| settings::get(cx));
            assert_eq!(now.web_previews_allowed(), expected);
            assert!(now.web_previews_answered(), "the switch is an answer too");
        }
    }
}
