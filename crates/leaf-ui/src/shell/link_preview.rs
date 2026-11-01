//! The card that previews a page when the pointer rests on a link to it (E10-S12), as Notion does:
//! where the page sits, its icon and title, and its first few blocks, so you can see it is the right
//! page before opening it. A link to a block shows that block highlighted among its neighbours.
//!
//! It waits [`OPEN_DELAY`] on a link before showing, and never shows while a mouse button is down. It
//! goes when the pointer leaves both the link and the card (after [`CLOSE_DELAY`], so the pointer can
//! cross to the card), and at once on a key press, a click, a scroll or Esc. Settings can turn it off.
//! What the card says is `leaf_editor`'s ([`PagePreview`]); this module only times, places and draws it.

use std::time::Duration;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Anchor, AnyElement, ClickEvent, Context, FontWeight, InteractiveElement as _, IntoElement,
    KeystrokeEvent, ListOffset, ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _,
    Task, Window, anchored, deferred, div, point, px,
};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit_assets::IconName;
use leaf_core::{BlockId, PageId};
use leaf_editor::{Destination, DrawnLink, LinkState, PagePreview, PreviewKind, PreviewLine};

use super::Shell;
use super::slash_popover::{Side, place};

/// How long the pointer rests on a link before the card shows.
pub(super) const OPEN_DELAY: Duration = Duration::from_millis(600);
/// How long the card stays once the pointer has left the link and the card, so it can cross between them.
pub(super) const CLOSE_DELAY: Duration = Duration::from_millis(200);

/// The card's width.
const CARD_WIDTH_PX: f32 = 320.0;

/// The link the pointer is on: which row, which link, and the line it hangs from.
#[derive(Debug, Clone, PartialEq)]
struct OnLink {
    row: BlockId,
    page: PageId,
    block: Option<BlockId>,
    /// Where the link's label starts, in characters drawn, so two links to one page are told apart.
    start: usize,
    x: Pixels,
    top: Pixels,
    bottom: Pixels,
}

/// The card on screen.
struct Shown {
    on: OnLink,
    preview: PagePreview,
    /// The page's scroll position when it opened: a scroll moves the link away, so the card goes.
    scroll: ListOffset,
}

/// What the page keeps about the card between frames.
#[derive(Default)]
pub(super) struct LinkPreviewState {
    /// The link under the pointer, while it is there.
    on: Option<OnLink>,
    /// Whether the pointer is over the card.
    over_card: bool,
    /// The pause before the card shows, or before it goes.
    timer: Option<Task<()>>,
    shown: Option<Shown>,
}

impl Shell {
    /// The pointer moved over `row` (from the row's mouse move): what is under it now. A button held
    /// down (selecting, dragging) closes the card and opens none.
    pub(super) fn link_preview_moved(
        &mut self,
        row: BlockId,
        position: gpui::Point<Pixels>,
        pressed: bool,
        cx: &mut Context<Self>,
    ) {
        if pressed {
            self.hide_link_preview(cx);
            return;
        }
        let on = self
            .blocks
            .painted
            .link_under(row, position)
            .filter(|(link, ..)| link.state != LinkState::Missing)
            .map(|(link, x, top, bottom)| on_link(row, &link, x, top, bottom));
        self.link_preview_at(on, cx);
    }

    /// The pointer left `row`.
    pub(super) fn link_preview_left(&mut self, row: BlockId, cx: &mut Context<Self>) {
        if self.blocks.preview.on.as_ref().is_some_and(|on| on.row == row) {
            self.link_preview_at(None, cx);
        }
    }

    /// The pointer is now on `on` (or on no link).
    fn link_preview_at(&mut self, on: Option<OnLink>, cx: &mut Context<Self>) {
        let state = &mut self.blocks.preview;
        if state.on == on {
            return;
        }
        if on.is_some() && !crate::settings::get(cx).link_previews() {
            state.on = None;
            return;
        }
        state.on = on;
        if let Some(on) = &state.on {
            if state.shown.as_ref().is_some_and(|shown| &shown.on != on) {
                state.shown = None;
                cx.notify();
            }
            if state.shown.is_none() {
                state.timer = Some(Self::after(OPEN_DELAY, cx, Self::show_link_preview));
            }
        } else if state.shown.is_some() {
            if !state.over_card {
                state.timer = Some(Self::after(CLOSE_DELAY, cx, Self::link_preview_settle));
            }
        } else {
            state.timer = None;
        }
    }

    /// Runs `then` on the shell after `delay`, unless the returned task is dropped first.
    fn after(
        delay: Duration,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) -> Task<()> {
        cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(delay).await;
            let _ = shell.update(cx, |shell, cx| then(shell, cx));
        })
    }

    /// The pause on a link is over: loads its page if it isn't yet, and shows the card. A link to a
    /// deleted page has no card.
    fn show_link_preview(&mut self, cx: &mut Context<Self>) {
        self.blocks.preview.timer = None;
        let Some(on) = self.blocks.preview.on.clone() else {
            return;
        };
        self.load_page_into_editor(on.page);
        let Some(preview) = self.editor.page_preview(on.page, on.block) else {
            return;
        };
        self.blocks.preview.shown = Some(Shown {
            on,
            preview,
            scroll: self.blocks.list.state.logical_scroll_top(),
        });
        cx.notify();
    }

    /// The pause after leaving: the card goes unless the pointer came back to the link or the card.
    fn link_preview_settle(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.blocks.preview;
        state.timer = None;
        if state.on.is_none() && !state.over_card && state.shown.take().is_some() {
            cx.notify();
        }
    }

    /// Closes the card and forgets the link under the pointer; `true` when a card was showing.
    pub(super) fn hide_link_preview(&mut self, cx: &mut Context<Self>) -> bool {
        let state = std::mem::take(&mut self.blocks.preview);
        let shown = state.shown.is_some();
        if shown {
            cx.notify();
        }
        shown
    }

    /// Where the first page link of row `index` of the open page is drawn, from the last frame. Used by the
    /// screenshot tool to rest the pointer on it.
    pub fn link_point(&mut self, index: usize) -> Option<gpui::Point<Pixels>> {
        let row = self.page()?.rows().ok()?.get(index)?.id;
        self.blocks.painted.first_link_middle(row)
    }

    /// Ends the pause on the link under the pointer now. For the screenshot tool, whose off-screen window
    /// runs no timers.
    pub fn show_link_preview_now(&mut self, cx: &mut Context<Self>) {
        self.show_link_preview(cx);
    }

    /// Whether the card is showing, and for which page.
    #[must_use]
    pub fn link_preview_page(&self) -> Option<PageId> {
        self.blocks.preview.shown.as_ref().map(|shown| shown.preview.page)
    }

    /// Closes the card on any key, before the key does anything else: bound keys (arrows, Esc) never
    /// reach the page's own key listeners. Esc that closes a card does nothing else.
    pub(super) fn close_preview_on_keys(cx: &mut Context<Self>) -> gpui::Subscription {
        let shell = cx.weak_entity();
        cx.intercept_keystrokes(move |event: &KeystrokeEvent, _, cx| {
            let closed = shell.update(cx, Self::hide_link_preview).unwrap_or(false);
            if closed && event.keystroke.key == "escape" {
                cx.stop_propagation();
            }
        })
    }

    /// The card, under the link's line (or over it near the window's bottom), if it is showing.
    pub(super) fn render_link_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let shown = self.blocks.preview.shown.as_ref()?;
        if shown.scroll != self.blocks.list.state.logical_scroll_top() {
            self.blocks.preview = LinkPreviewState::default();
            return None;
        }
        let preview = shown.preview.clone();
        let on = shown.on.clone();
        let (side, edge) = place(
            on.top,
            on.bottom,
            estimated_height(&preview),
            window.viewport_size().height,
        );
        let anchor = match side {
            Side::Below => Anchor::TopLeft,
            Side::Above => Anchor::BottomLeft,
        };
        let destination = Destination {
            page: preview.page,
            block: preview.block,
        };
        let card = Self::preview_card(&preview, cx)
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                shell.blocks.preview.over_card = *hovered;
                if !*hovered && shell.blocks.preview.on.is_none() {
                    shell.blocks.preview.timer =
                        Some(Self::after(CLOSE_DELAY, cx, Self::link_preview_settle));
                }
            }))
            .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                shell.hide_link_preview(cx);
                shell.open_at(destination, window, cx);
            }));
        Some(
            deferred(
                anchored()
                    .position(point(on.x - px(4.0), edge))
                    .anchor(anchor)
                    .snap_to_window_with_margin(px(8.0))
                    .child(card),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// The card's look: path, icon and title, then the lines.
    fn preview_card(preview: &PagePreview, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let path = (!preview.path.is_empty()).then(|| {
            div()
                .text_xs()
                .text_color(muted)
                .truncate()
                .child(preview.path.join(" / "))
        });
        let title = h_flex()
            .gap_1p5()
            .min_w_0()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .child(match &preview.icon {
                Some(icon) => div().flex_none().child(icon.clone()).into_any_element(),
                None => Icon::new(IconName::FileText)
                    .small()
                    .text_color(muted)
                    .into_any_element(),
            })
            .child(div().min_w_0().truncate().child(preview.title.clone()));
        let note = if preview.in_trash {
            Some("In trash")
        } else if preview.loaded && preview.lines.is_empty() {
            Some("Empty page")
        } else {
            None
        };
        v_flex()
            .id("link-preview")
            .debug_selector(|| "link-preview".into())
            .w(px(CARD_WIDTH_PX))
            .gap_1()
            .p_3()
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius)
            .shadow_lg()
            .cursor_pointer()
            .children(path)
            .child(title)
            .children(note.map(|note| div().text_xs().text_color(muted).child(note)))
            .when(!preview.lines.is_empty(), |card| {
                card.child(
                    v_flex()
                        .pt_1()
                        .gap_0p5()
                        .children(preview.lines.iter().map(|line| Self::preview_line(line, cx))),
                )
            })
            .when(preview.more, |card| {
                card.child(div().text_xs().text_color(muted).child("…"))
            })
    }

    /// One line, drawn like its block, smaller.
    fn preview_line(line: &PreviewLine, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let icon = |name: IconName| Icon::new(name).xsmall().text_color(muted).into_any_element();
        let marker: Option<AnyElement> = match &line.kind {
            PreviewKind::Bullet => Some(div().child("•").into_any_element()),
            PreviewKind::Number(n) => Some(div().child(format!("{n}.")).into_any_element()),
            PreviewKind::Todo(true) => Some(
                Icon::new(IconName::SquareCheck)
                    .xsmall()
                    .text_color(theme.primary)
                    .into_any_element(),
            ),
            PreviewKind::Todo(false) => Some(
                div()
                    .size(px(11.0))
                    .border_1()
                    .border_color(muted)
                    .rounded(px(2.0))
                    .into_any_element(),
            ),
            PreviewKind::Callout(tone) => Some(icon(super::callout::icon_of(tone))),
            PreviewKind::Image => Some(icon(IconName::Image)),
            PreviewKind::Table => Some(icon(IconName::Table)),
            PreviewKind::Bookmark => Some(icon(IconName::Globe)),
            PreviewKind::Database => Some(icon(IconName::Database)),
            _ => None,
        };
        let text = match (&line.kind, line.text.is_empty()) {
            (PreviewKind::Image, true) => "Image".to_owned(),
            (PreviewKind::Table, true) => "Table".to_owned(),
            (PreviewKind::Other, _) => "Unsupported block".to_owned(),
            _ => line.text.clone(),
        };
        #[expect(clippy::cast_precision_loss, reason = "the depth is at most 3")]
        let indent = px(12.0 * line.depth as f32);
        if line.kind == PreviewKind::Divider {
            return div()
                .ml(indent)
                .my_1()
                .h(px(1.0))
                .bg(theme.border)
                .into_any_element();
        }
        let words = div().flex_1().min_w_0().truncate().child(text);
        let words = match &line.kind {
            PreviewKind::Heading(level) => words
                .font_weight(FontWeight::SEMIBOLD)
                .when(*level <= 2, gpui::Styled::text_sm),
            PreviewKind::Code => words
                .font_family(theme.mono_font_family.clone())
                .bg(theme.muted)
                .px_1()
                .rounded(px(3.0)),
            PreviewKind::Quote => words.border_l_2().border_color(theme.border).pl_2(),
            PreviewKind::Todo(true) => words.text_color(muted).line_through(),
            PreviewKind::Image | PreviewKind::Table | PreviewKind::Other => words.text_color(muted),
            _ => words,
        };
        h_flex()
            .ml(indent)
            .gap_1p5()
            .min_w_0()
            .text_xs()
            .px_1()
            .rounded(px(3.0))
            .when(line.linked, |row| row.bg(theme.accent))
            .children(marker)
            .child(words)
            .into_any_element()
    }
}

/// What the pointer is on, from the painted link.
fn on_link(row: BlockId, link: &DrawnLink, x: Pixels, top: Pixels, bottom: Pixels) -> OnLink {
    OnLink {
        row,
        page: link.page,
        block: link.block,
        start: link.range.start,
        x,
        top,
        bottom,
    }
}

/// About how tall the card will be, to choose between below and above the line before it is drawn.
fn estimated_height(preview: &PagePreview) -> Pixels {
    let lines = preview.lines.len() + usize::from(preview.more) + usize::from(!preview.path.is_empty());
    #[expect(clippy::cast_precision_loss, reason = "a handful of lines")]
    let lines = lines as f32;
    px(56.0 + 20.0 * lines)
}

#[cfg(test)]
mod tests;
