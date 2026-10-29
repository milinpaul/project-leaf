//! The link popover (E07-S07, ADR 0052): Cmd/Ctrl+K over selected text opens a box under the selection
//! to type or paste an address. Enter saves it, Escape or a click elsewhere cancels, and Remove takes
//! the link off. Cmd/Ctrl+click on a link opens it in the browser (`rich_text`). Linking and the
//! addresses Leaf will open are `leaf_editor`'s; this module only draws the box and forwards.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, MouseDownEvent, ParentElement as _, Pixels, Point,
    Render, Styled as _, Subscription, Window, actions, anchored, deferred, point, px,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex};
use gpui_kit_assets::IconName;
use leaf_editor::{Selection, openable_url};

use super::{Shell, report};

/// The popover's key context, active while its field has the keyboard.
const CONTEXT: &str = "LeafLinkPopover";

actions!(
    leaf_link_popover,
    [
        /// Close the popover without saving.
        CancelLink
    ]
);

/// Binds Escape in the popover. Called once at startup.
pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CancelLink, Some(CONTEXT))]);
}

/// What the owner did in the popover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LinkChoice {
    /// Link the selection to this address (blank removes the link).
    Save(String),
    /// Take the link off the selection.
    Remove,
    /// Close without changing anything.
    Cancel,
}

/// The box: an address field, Open (for an address Leaf will open) and Remove (for an existing link).
pub(super) struct LinkPopover {
    input: Entity<InputState>,
    /// Whether the selection was already a link, so Remove has something to remove.
    linked: bool,
    _events: Subscription,
}

impl EventEmitter<LinkChoice> for LinkPopover {}

impl LinkPopover {
    /// A popover for a selection already linked to `href`, or not linked (`None`), with its address
    /// selected so typing replaces it.
    fn new(href: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let linked = href.is_some();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Paste or type a link")
                .default_value(href.unwrap_or_default())
        });
        let events = cx.subscribe_in(&input, window, |_, input, event, _, cx| match event {
            InputEvent::PressEnter { .. } => cx.emit(LinkChoice::Save(input.read(cx).value().to_string())),
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        Self {
            input,
            linked,
            _events: events,
        }
    }

    fn value(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }
}

impl Focusable for LinkPopover {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for LinkPopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let open = openable_url(&self.value(cx));
        h_flex()
            .id("link-popover")
            .debug_selector(|| "link-popover".into())
            .key_context(CONTEXT)
            .on_action(cx.listener(|_, _: &CancelLink, _, cx| cx.emit(LinkChoice::Cancel)))
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
                gpui::div().flex_1().child(
                    Input::new(&self.input).small().prefix(
                        Icon::new(IconName::Link)
                            .small()
                            .text_color(theme.muted_foreground),
                    ),
                ),
            )
            .child(
                Button::new("link-open")
                    .icon(IconName::ExternalLink)
                    .ghost()
                    .small()
                    .tooltip("Open link")
                    .disabled(open.is_none())
                    .on_click(move |_: &ClickEvent, _, cx| {
                        if let Some(url) = &open {
                            cx.open_url(url);
                        }
                    }),
            )
            .when(self.linked, |row| {
                row.child(
                    gpui::div().debug_selector(|| "link-remove".into()).child(
                        Button::new("link-remove")
                            .icon(IconName::Trash)
                            .ghost()
                            .small()
                            .tooltip("Remove link")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(LinkChoice::Remove))),
                    ),
                )
            })
    }
}

/// The open popover and where it goes.
pub(super) struct OpenLink {
    view: Entity<LinkPopover>,
    at: Point<Pixels>,
    _choice: Subscription,
}

impl Shell {
    /// Cmd/Ctrl+K over selected text: opens the popover under the start of the selection, filled in
    /// with the address when all of the selection is one link.
    pub fn open_link_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.page() else { return };
        let Ok(Some(Selection::Text { start, .. })) = page.selection() else {
            return;
        };
        page.close_block_menu();
        let href = page.active_marks().ok().and_then(|marks| marks.link);
        // The selection is in reading order: the box goes under where it starts.
        let at = self
            .blocks
            .painted
            .caret_bounds(start.block, start.at)
            .map_or_else(
                || point(px(0.0), px(0.0)),
                |(origin, height)| point(origin.x - px(4.0), origin.y + height + px(4.0)),
            );
        let view = cx.new(|cx| LinkPopover::new(href, window, cx));
        let choice = cx.subscribe_in(&view, window, |shell, _, choice: &LinkChoice, window, cx| {
            shell.link_chosen(choice, window, cx);
        });
        view.update(cx, |popover, cx| {
            popover.input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        });
        self.blocks.link = Some(OpenLink {
            view,
            at,
            _choice: choice,
        });
        cx.notify();
    }

    /// Whether the link popover is open.
    #[must_use]
    pub fn link_popover_open(&self) -> bool {
        self.blocks.link.is_some()
    }

    /// Acts on the popover's answer, closes it and gives the page the keyboard back with the
    /// selection as it was.
    fn link_chosen(&mut self, choice: &LinkChoice, window: &mut Window, cx: &mut Context<Self>) {
        self.blocks.link = None;
        let result = self.page().map(|page| match choice {
            LinkChoice::Save(href) => page.set_link(href),
            LinkChoice::Remove => page.remove_link(),
            LinkChoice::Cancel => Ok(()),
        });
        if let Some(Err(err)) = result {
            report("could not change the link", err);
        }
        self.focus_page(window, cx);
    }

    /// The popover, floating under the selection.
    pub(super) fn render_link_popover(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.blocks.link.as_ref()?;
        let view = open.view.clone();
        Some(
            deferred(
                anchored()
                    .position(open.at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        gpui::div()
                            .on_mouse_down_out(cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                                shell.link_chosen(&LinkChoice::Cancel, window, cx);
                            }))
                            .child(view),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests;
