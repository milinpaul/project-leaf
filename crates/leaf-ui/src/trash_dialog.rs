//! The Trash dialog: pages in the trash, newest first, with a filter field. Each row can be put back or
//! deleted for good (after a confirmation); clicking a row opens the page with a banner over it; the
//! foot empties the whole trash (after a confirmation naming how many pages go). Logic lives in
//! `leaf_editor::TrashList` and `WorkspaceEditor`; this draws them and forwards clicks to the shell.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable, FontWeight,
    HighlightStyle, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    SharedString, StatefulInteractiveElement as _, Styled, StyledText, Subscription, WeakEntity, Window, div,
    px,
};
use gpui_component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit_assets::IconName;
use leaf_core::PageId;
use leaf_editor::{TrashItem, TrashList, WorkspaceEditor, trashed_ago};

use crate::shell::Shell;

/// The dialog's width, in pixels.
const WIDTH: f32 = 560.0;

/// The list's height before it scrolls, in pixels.
const LIST_HEIGHT: f32 = 360.0;

/// The Trash dialog's content.
pub struct TrashPanel {
    list: TrashList,
    input: Entity<InputState>,
    shell: WeakEntity<Shell>,
    /// The time the list was read, for "5 min ago"; refreshed with the list.
    now: i64,
    _events: Subscription,
}

/// Opens the Trash dialog over `editor`'s trash; actions go to `shell`.
pub fn open(shell: WeakEntity<Shell>, editor: &WorkspaceEditor, window: &mut Window, cx: &mut gpui::App) {
    let items = editor.trash_items().unwrap_or_else(|err| {
        eprintln!("leaf: could not list the trash: {err}");
        Vec::new()
    });
    let panel = cx.new(|cx| TrashPanel::new(items, shell, window, cx));
    let focus = panel.read(cx).input.read(cx).focus_handle(cx);
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .p_0()
            .w(px(WIDTH))
            .margin_top(px(96.0))
            .close_button(false)
            .child(panel.clone())
    });
    window.focus(&focus, cx);
}

/// Asks before deleting `page` (titled `title`) for good, then deletes it and refreshes `panel` (when
/// the question came from the Trash dialog rather than the banner).
pub fn confirm_delete(
    shell: WeakEntity<Shell>,
    page: PageId,
    title: &str,
    panel: Option<WeakEntity<TrashPanel>>,
    window: &mut Window,
    cx: &mut gpui::App,
) {
    let description = format!("“{title}” and any pages inside it will be deleted. This can’t be undone.");
    window.open_alert_dialog(cx, move |alert, _, _| {
        let (shell, panel) = (shell.clone(), panel.clone());
        alert
            .title("Delete forever?")
            .description(description.clone())
            .ok_text("Delete forever")
            .ok_variant(ButtonVariant::Danger)
            .show_cancel(true)
            .on_ok(move |_, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.delete_page_forever(page, cx));
                if let Some(panel) = &panel {
                    let _ = panel.update(cx, TrashPanel::reload);
                }
                true
            })
    });
}

impl TrashPanel {
    fn new(
        items: Vec<TrashItem>,
        shell: WeakEntity<Shell>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter pages in the trash…"));
        let events = cx.subscribe_in(&input, window, |panel, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().to_string();
                panel.list.set_query(&query);
                cx.notify();
            }
        });
        Self {
            list: TrashList::new(items),
            input,
            shell,
            now: crate::shell::now_ms(),
            _events: events,
        }
    }

    /// Reads the trash again after something changed it, keeping the filter.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let items = self
            .shell
            .upgrade()
            .and_then(|shell| shell.read(cx).editor().trash_items().ok())
            .unwrap_or_default();
        let query = self.list.query().to_owned();
        self.list = TrashList::new(items);
        self.list.set_query(&query);
        self.now = crate::shell::now_ms();
        cx.notify();
    }

    fn restore(&mut self, page: PageId, cx: &mut Context<Self>) {
        let _ = self.shell.update(cx, |shell, cx| shell.restore_page(page, cx));
        self.reload(cx);
    }

    fn confirm_empty(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.list.total();
        if count == 0 {
            return;
        }
        let (shell, panel) = (self.shell.clone(), cx.entity().downgrade());
        let pages = if count == 1 {
            "1 page".to_owned()
        } else {
            format!("{count} pages")
        };
        let description = format!("{pages} and any pages inside them will be deleted. This can’t be undone.");
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, panel) = (shell.clone(), panel.clone());
            alert
                .title("Empty the trash?")
                .description(description.clone())
                .ok_text("Empty trash")
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    let _ = shell.update(cx, Shell::empty_trash);
                    let _ = panel.update(cx, TrashPanel::reload);
                    true
                })
        });
    }

    /// Opens a trashed page (with its banner) and closes the dialog.
    fn open_page(&mut self, page: PageId, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        let _ = self.shell.update(cx, |shell, cx| shell.open(page, window, cx));
    }

    fn render_row(
        &self,
        ix: usize,
        item: &TrashItem,
        highlights: &[std::ops::Range<usize>],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (muted, hover, text, danger, radius) = (
            theme.muted_foreground,
            theme.secondary_hover,
            theme.foreground,
            theme.danger,
            theme.radius,
        );
        let page = item.page;
        let label = item.label.clone();
        let bold = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            color: Some(text),
            ..HighlightStyle::default()
        };
        let title = StyledText::new(item.label.clone())
            .with_highlights(highlights.iter().map(|range| (range.clone(), bold)));
        let icon = match &item.icon {
            Some(emoji) => div().child(emoji.clone()).into_any_element(),
            None => Icon::new(IconName::FileText)
                .small()
                .text_color(muted)
                .into_any_element(),
        };
        let place = if item.path.is_empty() {
            SharedString::from(trashed_ago(self.now, item.trashed_at))
        } else {
            SharedString::from(format!(
                "in {} · {}",
                item.path,
                trashed_ago(self.now, item.trashed_at)
            ))
        };
        let restore = Button::new(("restore", ix))
            .debug_selector(move || format!("restore-{ix}"))
            .icon(Icon::new(IconName::RotateCcw))
            .ghost()
            .xsmall()
            .tooltip("Restore")
            .on_click(cx.listener(move |panel, _: &ClickEvent, _, cx| panel.restore(page, cx)));
        let delete = Button::new(("delete", ix))
            .debug_selector(move || format!("delete-{ix}"))
            .icon(Icon::new(IconName::Trash).text_color(danger))
            .ghost()
            .xsmall()
            .tooltip("Delete forever")
            .on_click(cx.listener(move |panel, _: &ClickEvent, window, cx| {
                let panel_handle = cx.entity().downgrade();
                confirm_delete(panel.shell.clone(), page, &label, Some(panel_handle), window, cx);
            }));
        h_flex()
            .id(("trash-row", ix))
            .group("trash-row")
            .debug_selector(move || format!("trash-row-{ix}"))
            .w_full()
            .h(px(48.0))
            .px_3()
            .gap_3()
            .rounded(radius)
            .hover(|style| style.bg(hover))
            .child(div().flex_none().w(px(20.0)).flex().justify_center().child(icon))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().text_sm().child(title))
                    .child(div().truncate().text_xs().text_color(muted).child(place)),
            )
            .child(
                h_flex()
                    .gap_1()
                    // The buttons' clicks are theirs, not the row's (which opens the page).
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .opacity(0.0)
                    .group_hover("trash-row", |style| style.opacity(1.0))
                    .child(restore)
                    .child(delete),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |panel, _: &MouseDownEvent, window, cx| panel.open_page(page, window, cx)),
            )
            .into_any_element()
    }
}

impl Focusable for TrashPanel {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for TrashPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, border) = (theme.muted_foreground, theme.border);
        let total = self.list.total();
        let shown: Vec<(TrashItem, Vec<std::ops::Range<usize>>)> = self
            .list
            .shown()
            .map(|(item, ranges)| (item.clone(), ranges.to_vec()))
            .collect();
        let body = if shown.is_empty() {
            let message = if total == 0 {
                "The trash is empty"
            } else {
                "No pages in the trash match"
            };
            div()
                .h(px(120.0))
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(muted)
                .child(message)
                .into_any_element()
        } else {
            v_flex()
                .id("trash-list")
                .max_h(px(LIST_HEIGHT))
                .overflow_y_scroll()
                .px_2()
                .children(
                    shown
                        .iter()
                        .enumerate()
                        .map(|(ix, (item, ranges))| self.render_row(ix, item, ranges, cx)),
                )
                .into_any_element()
        };
        let count = match total {
            0 => "No pages".to_owned(),
            1 => "1 page".to_owned(),
            n => format!("{n} pages"),
        };
        let empty = Button::new("empty-trash")
            .debug_selector(|| "empty-trash".into())
            .label("Empty trash")
            .danger()
            .small()
            .disabled(total == 0)
            .on_click(cx.listener(|panel, _: &ClickEvent, window, cx| panel.confirm_empty(window, cx)));
        v_flex()
            .child(
                h_flex()
                    .h(px(52.0))
                    .px_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(border)
                    .child(Icon::new(IconName::Trash).text_color(muted))
                    .child(
                        div().flex_1().child(
                            Input::new(&self.input)
                                .appearance(false)
                                .focus_bordered(false)
                                .px_0(),
                        ),
                    ),
            )
            .child(div().py_2().child(body))
            .child(
                h_flex()
                    .h(px(48.0))
                    .px_4()
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child(div().flex_1().child(count))
                    .when(total > 0, |foot| foot.child(empty)),
            )
    }
}

#[cfg(test)]
mod tests;
