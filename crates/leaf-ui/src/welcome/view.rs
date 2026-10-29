//! Drawing the welcome screen: a calm card on the page colour. On the left, the app mark, name and
//! tagline with the two actions; on the right, recent workspaces. In a narrow window the two stack.
//! The card fades up into place when it appears.

use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Animation, AnimationExt as _, AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement as _, Styled, StyledImage as _, Window, div, ease_out_quint, img, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;

use super::{Welcome, model};
use crate::settings_dialog::APP_ICON;

/// Below this window width the two columns stack.
const NARROW: f32 = 760.0;

/// The card's widest.
const CARD_WIDTH: f32 = 880.0;

impl Render for Welcome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = f32::from(window.viewport_size().width) < NARROW;
        let left = self.render_start(cx);
        let right = self.render_recent(cx);
        let theme = cx.theme();
        let (page, card, border, text) = (theme.sidebar, theme.background, theme.border, theme.foreground);
        let columns = if narrow {
            v_flex().gap_8().child(left).child(right).into_any_element()
        } else {
            h_flex()
                .items_start()
                .gap_10()
                .child(div().w(px(340.0)).flex_none().child(left))
                .child(div().flex_1().min_w_0().child(right))
                .into_any_element()
        };
        div()
            .id("welcome")
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .overflow_y_scroll()
            .bg(page)
            .text_color(text)
            .flex()
            .items_center()
            .justify_center()
            .p_6()
            .child(
                div()
                    .w_full()
                    .max_w(px(CARD_WIDTH))
                    .p_10()
                    .rounded(px(16.0))
                    .bg(card)
                    .border_1()
                    .border_color(border)
                    .shadow_lg()
                    .child(columns)
                    .with_animation(
                        "welcome-in",
                        Animation::new(Duration::from_millis(280)).with_easing(ease_out_quint()),
                        |card, done| card.opacity(done).mt(px(16.0 * (1.0 - done))),
                    ),
            )
    }
}

impl Welcome {
    /// The app mark, name, tagline, the two actions and any message.
    fn render_start(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let brand = v_flex()
            .gap_2()
            .child(
                img(APP_ICON.clone())
                    .size(px(56.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .pt_2()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .child("Leaf"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("Your notes and pages, kept on your own computer."),
            );
        let body = if self.create.is_some() {
            self.render_create_form(cx)
        } else {
            Self::render_actions(cx)
        };
        v_flex()
            .gap_8()
            .child(brand)
            .child(body)
            .children(self.render_message(cx))
            .into_any_element()
    }

    fn render_actions(cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_2()
            .child(
                action_row(
                    "create-workspace",
                    IconName::Plus,
                    "Create new workspace",
                    "A fresh folder for your pages",
                    cx,
                )
                .on_click(
                    cx.listener(|welcome, _: &ClickEvent, window, cx| welcome.start_create(window, cx)),
                ),
            )
            .child(
                action_row(
                    "open-workspace",
                    IconName::FolderOpen,
                    "Open existing folder",
                    "A .leaf folder on this computer",
                    cx,
                )
                .on_click(cx.listener(|welcome, _: &ClickEvent, _, cx| {
                    welcome.choose_folder_to_open(cx);
                })),
            )
            .into_any_element()
    }

    fn render_create_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(form) = &self.create else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let home = std::env::home_dir();
        let location = model::short_path(&form.location, home.as_deref());
        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("New workspace"),
            )
            .child(Input::new(&form.name))
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(muted)
                    .child(Icon::new(IconName::FolderOpen).xsmall())
                    .child(div().flex_1().min_w_0().truncate().child(location))
                    .child(
                        Button::new("change-location")
                            .label("Change…")
                            .ghost()
                            .xsmall()
                            .on_click(
                                cx.listener(|welcome, _: &ClickEvent, _, cx| welcome.choose_location(cx)),
                            ),
                    ),
            )
            .children(
                form.problem
                    .clone()
                    .map(|problem| div().text_xs().text_color(danger).child(problem)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("create-confirm")
                            .debug_selector(|| "create-confirm".into())
                            .label("Create workspace")
                            .primary()
                            .on_click(
                                cx.listener(|welcome, _: &ClickEvent, _, cx| welcome.submit_create(cx)),
                            ),
                    )
                    .child(
                        Button::new("create-cancel")
                            .label("Cancel")
                            .ghost()
                            .on_click(cx.listener(|welcome, _: &ClickEvent, window, cx| {
                                welcome.cancel_create(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// A problem or question under the actions.
    fn render_message(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let message = self.message.clone()?;
        let theme = cx.theme();
        let (text, bg, radius) = (theme.foreground, theme.secondary, theme.radius);
        let create_here = message.create_here.map(|folder| {
            Button::new("create-here")
                .debug_selector(|| "create-here".into())
                .label("Create workspace here")
                .primary()
                .small()
                .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                    cx.emit(super::WelcomeEvent::Create(folder.clone()));
                }))
        });
        Some(
            v_flex()
                .debug_selector(|| "welcome-message".into())
                .gap_2()
                .p_3()
                .rounded(radius)
                .bg(bg)
                .text_sm()
                .text_color(text)
                .child(message.text)
                .children(create_here)
                .into_any_element(),
        )
    }

    /// Recent workspaces, or a quiet note when there are none yet.
    fn render_recent(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let heading = div()
            .pb_2()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(muted)
            .child("Recent");
        let body = if self.recent.is_empty() {
            div()
                .py_6()
                .text_sm()
                .text_color(muted)
                .child("Workspaces you open will appear here.")
                .into_any_element()
        } else {
            v_flex()
                .gap_1()
                .children((0..self.recent.len()).map(|ix| self.render_recent_row(ix, cx)))
                .into_any_element()
        };
        v_flex().child(heading).child(body).into_any_element()
    }

    fn render_recent_row(&self, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let entry = &self.recent[ix];
        let theme = cx.theme();
        let (muted, hover, selected_bg, radius) = (
            theme.muted_foreground,
            theme.secondary_hover,
            theme.sidebar_accent,
            theme.radius,
        );
        let home = std::env::home_dir();
        let detail: SharedString = if entry.missing {
            "Can't be found".into()
        } else {
            format!(
                "{}  ·  {}",
                model::short_path(&entry.path, home.as_deref()),
                model::opened_label(self.now, entry.opened_at)
            )
            .into()
        };
        let initial = entry
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        let accent = theme.primary;
        let badge = div()
            .flex_none()
            .size(px(32.0))
            .rounded(radius)
            .flex()
            .items_center()
            .justify_center()
            .bg(accent.opacity(0.14))
            .text_color(accent)
            .font_weight(FontWeight::BOLD)
            .child(initial);
        let actions = entry.missing.then(|| {
            h_flex()
                .gap_1()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new(("locate", ix))
                        .debug_selector(move || format!("locate-{ix}"))
                        .label("Locate…")
                        .ghost()
                        .xsmall()
                        .on_click(
                            cx.listener(move |welcome, _: &ClickEvent, _, cx| welcome.locate_recent(ix, cx)),
                        ),
                )
                .child(
                    Button::new(("remove", ix))
                        .debug_selector(move || format!("remove-{ix}"))
                        .label("Remove")
                        .ghost()
                        .xsmall()
                        .on_click(
                            cx.listener(move |welcome, _: &ClickEvent, _, cx| welcome.remove_recent(ix, cx)),
                        ),
                )
        });
        h_flex()
            .id(("recent", ix))
            .debug_selector(move || format!("recent-{ix}"))
            .h(px(56.0))
            .px_3()
            .gap_3()
            .rounded(radius)
            .when(!entry.missing, |row| row.hover(|style| style.bg(hover)))
            .when(self.selected == Some(ix), |row| row.bg(selected_bg))
            .when(entry.missing, |row| row.opacity(0.55))
            .child(badge)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(entry.name.clone()),
                    )
                    .child(div().truncate().text_xs().text_color(muted).child(detail)),
            )
            .children(actions)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |welcome, _: &MouseDownEvent, _, cx| welcome.open_recent(ix, cx)),
            )
            .into_any_element()
    }
}

/// A large action: icon, title and a line saying what it does.
fn action_row(
    id: &'static str,
    icon: IconName,
    title: &'static str,
    detail: &'static str,
    cx: &mut Context<Welcome>,
) -> gpui::Stateful<gpui::Div> {
    let theme = cx.theme();
    let (muted, hover, accent, radius, border) = (
        theme.muted_foreground,
        theme.secondary_hover,
        theme.primary,
        theme.radius,
        theme.border,
    );
    h_flex()
        .id(id)
        .debug_selector(move || id.to_owned())
        .gap_3()
        .p_3()
        .rounded(radius)
        .border_1()
        .border_color(border)
        .hover(|style| style.bg(hover))
        .child(
            div()
                .flex_none()
                .size(px(36.0))
                .rounded(radius)
                .flex()
                .items_center()
                .justify_center()
                .bg(accent.opacity(0.12))
                .text_color(accent)
                .child(Icon::new(icon)),
        )
        .child(
            v_flex()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title))
                .child(div().text_xs().text_color(muted).child(detail)),
        )
}
