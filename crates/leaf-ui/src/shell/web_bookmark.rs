//! Making web bookmarks (E15-S03, ADR-XXX-E15-S03): the small menu under a pasted web address
//! ("Dismiss", "Create web bookmark") and the address box the slash menu's "Web bookmark" opens. What is
//! offered, where a bookmark goes and which addresses it takes are `leaf_editor`'s; this module draws
//! the two and forwards. The page keeps the keyboard while the menu shows, so Up, Down, Enter and Escape
//! drive it and any other key closes it; the box takes the keyboard until Enter, Escape or a click away.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, MouseButton, MouseDownEvent, ParentElement as _,
    Pixels, Point, Render, Styled as _, Subscription, Window, actions, anchored, deferred, div, point, px,
};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{BOOKMARK_OFFER_CHOICES, PageEditor};

use super::{Shell, report};

/// The address box's key context, active while its field has the keyboard.
const CONTEXT: &str = "LeafWebAddress";

/// One menu row's height.
const ROW_PX: f32 = 32.0;

actions!(
    leaf_web_address,
    [
        /// Close the address box without adding a bookmark.
        CancelWebAddress
    ]
);

/// Binds Escape in the address box. Called once at startup.
pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelWebAddress, Some(CONTEXT))]);
}

/// What the owner did in the address box.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AddressChoice {
    /// Add a bookmark for what was typed.
    Add(String),
    /// Close without adding one.
    Cancel,
}

/// The address box: one field, and a hint while what is typed can't be a web bookmark's address.
pub(super) struct AddressPrompt {
    input: Entity<InputState>,
    _events: Subscription,
}

impl EventEmitter<AddressChoice> for AddressPrompt {}

impl AddressPrompt {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Paste or type a web address"));
        let events = cx.subscribe_in(&input, window, |_, input, event, _, cx| match event {
            InputEvent::PressEnter { .. } => cx.emit(AddressChoice::Add(input.read(cx).value().to_string())),
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        Self {
            input,
            _events: events,
        }
    }
}

impl Focusable for AddressPrompt {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for AddressPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let typed = self.input.read(cx).value().to_string();
        let refused = !typed.trim().is_empty() && PageEditor::web_address(&typed).is_none();
        v_flex()
            .id("web-address")
            .debug_selector(|| "web-address".into())
            .key_context(CONTEXT)
            .on_action(cx.listener(|_, _: &CancelWebAddress, _, cx| cx.emit(AddressChoice::Cancel)))
            .w(px(380.0))
            .gap_1()
            .p_1()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .shadow_lg()
            .child(
                Input::new(&self.input).small().prefix(
                    Icon::new(IconName::Bookmark)
                        .small()
                        .text_color(theme.muted_foreground),
                ),
            )
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .text_xs()
                    .text_color(if refused {
                        theme.danger
                    } else {
                        theme.muted_foreground
                    })
                    .child(if refused {
                        "Not a web address: it needs to start with http:// or https://, or look like leaf.app"
                    } else {
                        "Press Enter to add a web bookmark"
                    }),
            )
    }
}

/// The open address box and where it goes.
pub(super) struct OpenAddress {
    view: Entity<AddressPrompt>,
    at: Point<Pixels>,
    _choice: Subscription,
}

impl Shell {
    /// Shows the address box if the page just asked for one (the slash menu's "Web bookmark"), under
    /// where `/web bookmark` was typed.
    pub(super) fn ask_web_address_if_asked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.page().is_some_and(PageEditor::take_web_address_request) {
            return;
        }
        let place = self
            .page()
            .and_then(|page| page.web_address_request_at().ok().flatten());
        let at = place
            .and_then(|caret| self.blocks.painted.caret_bounds(caret.block, caret.at))
            .map_or_else(
                || point(px(0.0), px(0.0)),
                |(origin, height)| point(origin.x - px(4.0), origin.y + height + px(4.0)),
            );
        let view = cx.new(|cx| AddressPrompt::new(window, cx));
        let choice = cx.subscribe_in(&view, window, |shell, _, choice: &AddressChoice, window, cx| {
            shell.address_chosen(choice, window, cx);
        });
        view.update(cx, |prompt, cx| {
            prompt.input.update(cx, |input, cx| input.focus(window, cx));
        });
        self.blocks.address = Some(OpenAddress {
            view,
            at,
            _choice: choice,
        });
        cx.notify();
    }

    /// Acts on the box's answer. An address that can't be a bookmark's keeps the box open, saying so.
    fn address_chosen(&mut self, choice: &AddressChoice, window: &mut Window, cx: &mut Context<Self>) {
        if let AddressChoice::Add(typed) = choice {
            if PageEditor::web_address(typed).is_none() {
                return;
            }
            let added = self.page().map(|page| page.insert_web_bookmark(typed));
            if let Some(Err(err)) = added {
                report("could not add the web bookmark", err);
            }
        } else if let Some(page) = self.page() {
            page.cancel_web_address();
        }
        self.blocks.address = None;
        self.focus_page(window, cx);
        self.changed(window, cx);
    }

    /// The address box, floating under where it was asked for.
    pub(super) fn render_address_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.blocks.address.as_ref()?;
        let view = open.view.clone();
        Some(
            deferred(
                anchored()
                    .position(open.at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        div()
                            .on_mouse_down_out(cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                                shell.address_chosen(&AddressChoice::Cancel, window, cx);
                            }))
                            .child(view),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// The offer under a pasted web address, while it stands and the page has the keyboard.
    pub(super) fn render_bookmark_offer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.page_focus.is_focused(window) {
            return None;
        }
        let offer = self.page()?.bookmark_offer().ok().flatten()?;
        let (origin, line) = self.blocks.painted.caret_bounds(offer.at.block, offer.at.at)?;
        let at = point(origin.x - px(8.0), origin.y + line + px(4.0));
        let theme = cx.theme();
        let (bg, border, accent, muted, radius) = (
            theme.popover,
            theme.border,
            theme.accent,
            theme.muted_foreground,
            theme.radius,
        );
        let rows: Vec<AnyElement> = BOOKMARK_OFFER_CHOICES
            .iter()
            .enumerate()
            .map(|(ix, label)| {
                let icon = if ix == 0 { IconName::X } else { IconName::Bookmark };
                h_flex()
                    .id(("bookmark-offer", ix))
                    .debug_selector(move || format!("bookmark-offer-{ix}"))
                    .h(px(ROW_PX))
                    .px_2()
                    .gap_2()
                    .rounded(radius)
                    .cursor_pointer()
                    .when(ix == offer.selected, |row| row.bg(accent))
                    .child(Icon::new(icon).small().text_color(muted))
                    .child(*label)
                    .on_mouse_move(cx.listener(move |shell, _, _, cx| {
                        if let Some(page) = shell.page() {
                            page.bookmark_offer_highlight(ix);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            if let Some(Err(err)) =
                                shell.page().map(|page| page.bookmark_offer_apply(Some(ix)))
                            {
                                report("could not make the web bookmark", err);
                            }
                            shell.changed(window, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        Some(
            deferred(
                anchored().position(at).snap_to_window_with_margin(px(8.0)).child(
                    v_flex()
                        .id("bookmark-offer")
                        .debug_selector(|| "bookmark-offer".into())
                        .w(px(220.0))
                        .p_1()
                        .bg(bg)
                        .border_1()
                        .border_color(border)
                        .rounded(radius)
                        .shadow_lg()
                        .text_sm()
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
