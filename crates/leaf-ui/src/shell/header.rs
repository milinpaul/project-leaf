//! The open page's header: its emoji icon (or an "Add icon" button that shows on hover) above the title,
//! which reads as the page's heading. Clicking the icon opens the icon picker in a popover under it.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Anchor, AnyElement, AppContext as _, Context, Focusable as _, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement, Styled, Window, div, px, rems,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, v_flex};
use gpui_kit_assets::IconName;

use super::{Shell, report};
use crate::icon_picker::{IconPicked, IconPickerPanel};
use crate::theme;

/// The page icon's size above the title, in pixels.
const ICON_SIZE: f32 = 56.0;

impl Shell {
    /// The icon (or "Add icon") and the title of the open page.
    pub(super) fn render_page_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let icon = self.editor.selected_icon().unwrap_or_else(|err| {
            report("could not read the page icon", err);
            None
        });
        let open = self.icon_picker.is_some();
        let muted = cx.theme().muted_foreground;
        let trigger = match &icon {
            Some(emoji) => Button::new("page-icon")
                .debug_selector(|| "page-icon".into())
                .ghost()
                // Room for the large emoji; a button is sized for a line of text.
                .h(px(ICON_SIZE + 16.0))
                .px_1()
                .tooltip("Change icon")
                .child(
                    div()
                        .text_size(px(ICON_SIZE))
                        .line_height(px(ICON_SIZE + 8.0))
                        .child(emoji.clone()),
                ),
            None => Button::new("add-icon")
                .debug_selector(|| "add-icon".into())
                .ghost()
                .small()
                .text_color(muted)
                .icon(Icon::new(IconName::FaceSlightlySmilingPlus))
                .label("Add icon"),
        };
        let shell = cx.entity().downgrade();
        let mut popover = Popover::new("page-icon-picker")
            .anchor(Anchor::TopLeft)
            .trigger(trigger)
            .open(open)
            .on_open_change(move |open, window, cx| {
                let open = *open;
                let _ = shell.update(cx, |shell, cx| {
                    if open {
                        shell.open_icon_picker(window, cx);
                    } else {
                        shell.icon_picker = None;
                        cx.notify();
                    }
                });
            });
        if let Some((panel, _)) = &self.icon_picker {
            popover = popover.child(panel.clone());
        }
        // Without an icon, "Add icon" stays out of the way until the pointer is over the header.
        let hidden_until_hover = icon.is_none() && !open;
        let icon_row = div()
            .flex()
            .min_h(px(28.0))
            .when(hidden_until_hover, |row| {
                row.opacity(0.0)
                    .group_hover("page-header", |style| style.opacity(1.0))
            })
            .child(popover);
        v_flex()
            .id("page-header")
            .group("page-header")
            .gap_1()
            .child(icon_row)
            // The title reads as the page's heading, not a form field: no box, large and bold.
            .child(
                Input::new(&self.title)
                    .appearance(false)
                    .focus_bordered(false)
                    .text_size(px(theme::TITLE_SIZE))
                    .font_weight(FontWeight::BOLD)
                    // The input's own height and line height fit body text; size them to the heading.
                    .line_height(rems(2.75))
                    .h(rems(3.25))
                    .px_0(),
            )
            .into_any_element()
    }

    /// Opens the icon picker for the open page, with the cursor in its search field.
    fn open_icon_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let has_icon = matches!(self.editor.selected_icon(), Ok(Some(_)));
        let panel = cx.new(|cx| IconPickerPanel::new(has_icon, window, cx));
        let events = cx.subscribe(&panel, |shell, _, picked: &IconPicked, cx| {
            let icon = match picked {
                IconPicked::Emoji(emoji) => Some(*emoji),
                IconPicked::Remove => None,
            };
            if let Some(page) = shell.editor.selected()
                && let Err(err) = shell.editor.set_icon(page, icon)
            {
                report("could not change the page icon", err);
            }
            shell.icon_picker = None;
            cx.notify();
        });
        window.focus(&panel.read(cx).focus_handle(cx), cx);
        self.icon_picker = Some((panel, events));
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
