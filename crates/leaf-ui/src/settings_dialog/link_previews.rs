//! Settings › Appearance › "Preview pages on hover" (E10-S12): whether resting the pointer on a link to a
//! page shows a card with that page's start. On unless turned off here; some find a card popping up over a
//! page full of links distracting. The choice is the device's (`settings.json`), not the workspace's.

use gpui::{AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _, Styled as _, div};
use gpui_component::switch::Switch;
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

use super::{SettingsPanel, card};
use crate::settings;

impl SettingsPanel {
    /// The switch, with what it does.
    pub(super) fn link_previews(cx: &mut Context<Self>) -> AnyElement {
        let on = settings::get(cx).link_previews();
        let muted = cx.theme().muted_foreground;
        card(cx)
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
                                    .child("Preview pages on hover"),
                            )
                            .child(div().text_xs().text_color(muted).child(
                                "Rest the pointer on a link to a page to see its first lines before opening it.",
                            )),
                    )
                    .child(
                        div()
                            .flex_none()
                            .debug_selector(|| "link-previews".into())
                            .child(Switch::new("link-previews").checked(on).on_click(cx.listener(
                                |_, checked: &bool, _, cx| {
                                    settings::update(cx, |s| s.set_link_previews(*checked));
                                    settings::save(cx);
                                    cx.notify();
                                },
                            ))),
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
    fn the_switch_turns_link_previews_off_and_on(cx: &mut TestAppContext) {
        cx.update(|cx| crate::init(None, cx));
        let (_panel, cx) = cx.add_window_view(|_, cx| SettingsPanel {
            section: Section::Appearance,
            workspace: WorkspaceInfo::default(),
            focus: cx.focus_handle(),
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.update(|_, cx| settings::get(cx).link_previews()),
            "on by default"
        );
        for expected in [false, true] {
            let at = cx.debug_bounds("link-previews").unwrap().center();
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            assert_eq!(cx.update(|_, cx| settings::get(cx).link_previews()), expected);
        }
    }
}
