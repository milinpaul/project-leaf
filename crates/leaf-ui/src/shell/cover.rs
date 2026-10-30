//! The open page's cover: a full-width band above the page header, with Change cover and Remove on
//! hover, and the cover picker in a popover. Built-in covers are SVG files drawn with `img()`; a cover
//! this version doesn't know shows as a plain band, so it can still be seen and removed (ADR 0045).

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Anchor, AnyElement, AppContext as _, ClickEvent, Context, Focusable as _, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement, Styled, StyledImage as _, Window, div, img, px,
};
use gpui_component::button::Button;
use gpui_component::popover::Popover;
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex};
use leaf_editor::{BuiltinCover, PageCover};

use super::{Shell, report};
use crate::cover_picker::{CoverPicked, CoverPickerPanel};
use crate::covers;

/// The cover band's shape: its height follows its width, so a wide window crops less off the top and
/// bottom of the picture and a narrow one less off its sides (covers are drawn at 4:1).
const COVER_ASPECT: f32 = 3.5;

/// The band's height limits, in pixels: never a sliver, never most of the screen.
const COVER_HEIGHT: (f32, f32) = (160.0, 320.0);

impl Shell {
    /// The open page's cover, if it has one. A read error is logged and shows as no cover.
    pub(super) fn page_cover(&self) -> Option<PageCover> {
        self.editor.selected_cover().unwrap_or_else(|err| {
            report("could not read the page cover", err);
            None
        })
    }

    /// The cover band, full width, with Change cover and Remove in its corner while the pointer is
    /// over it.
    pub(super) fn render_cover(&mut self, cover: &PageCover, cx: &mut Context<Self>) -> AnyElement {
        let picture = match cover {
            PageCover::Builtin(cover) => img(covers::asset_path(*cover))
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            PageCover::Unknown(_) => div().size_full().bg(cx.theme().muted).into_any_element(),
        };
        let open = self.cover_picker.is_some();
        let change = crate::theme::button("change-cover")
            .debug_selector(|| "change-cover".into())
            .small()
            .label("Change cover");
        let remove = crate::theme::button("remove-cover")
            .debug_selector(|| "remove-cover".into())
            .small()
            .label("Remove")
            .on_click(cx.listener(|shell, _: &ClickEvent, _, cx| shell.set_page_cover(None, cx)));
        let controls = h_flex()
            .absolute()
            .right(px(16.0))
            .bottom(px(12.0))
            .gap_1()
            // Out of the way until the pointer is over the cover, unless the picker is open.
            .when(!open, |row| {
                row.opacity(0.0)
                    .group_hover("page-cover", |style| style.opacity(1.0))
            })
            .child(self.cover_popover(change, cx))
            .child(remove);
        div()
            .id("page-cover")
            .debug_selector(|| "page-cover".into())
            .group("page-cover")
            .relative()
            .w_full()
            .overflow_hidden()
            // An invisible spacer sets the height: an aspect ratio on the band itself would narrow it
            // to keep its shape once the height hits its limit, instead of spanning the page.
            .child(
                div()
                    .w_full()
                    .aspect_ratio(COVER_ASPECT)
                    .min_h(px(COVER_HEIGHT.0))
                    .max_h(px(COVER_HEIGHT.1)),
            )
            .child(div().absolute().inset_0().child(picture))
            .child(controls)
            .into_any_element()
    }

    /// The cover picker in a popover that `trigger` opens: "Add cover" in the header, or "Change cover"
    /// on the band.
    pub(super) fn cover_popover(&mut self, trigger: Button, cx: &mut Context<Self>) -> Popover {
        let shell = cx.entity().downgrade();
        let mut popover = Popover::new("page-cover-picker")
            .anchor(Anchor::TopLeft)
            .trigger(trigger)
            .open(self.cover_picker.is_some())
            .on_open_change(move |open, window, cx| {
                let open = *open;
                let _ = shell.update(cx, |shell, cx| {
                    if open {
                        shell.open_cover_picker(window, cx);
                    } else {
                        shell.cover_picker = None;
                        cx.notify();
                    }
                });
            });
        if let Some((panel, _)) = &self.cover_picker {
            popover = popover.child(panel.clone());
        }
        popover
    }

    /// Opens the cover picker, with the keyboard in it so Esc closes it.
    fn open_cover_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.page_cover();
        let panel = cx.new(|cx| CoverPickerPanel::new(current, cx));
        let events = cx.subscribe(&panel, |shell, _, picked: &CoverPicked, cx| {
            match picked {
                CoverPicked::Cover(cover) => shell.set_page_cover(Some(*cover), cx),
                CoverPicked::Remove => shell.set_page_cover(None, cx),
                CoverPicked::Dismissed => {}
            }
            shell.cover_picker = None;
            cx.notify();
        });
        window.focus(&panel.read(cx).focus_handle(cx), cx);
        self.cover_picker = Some((panel, events));
        cx.notify();
    }

    /// Gives the open page `cover`, or removes its cover with `None`.
    fn set_page_cover(&mut self, cover: Option<BuiltinCover>, cx: &mut Context<Self>) {
        if let Some(page) = self.editor.selected()
            && let Err(err) = self.editor.set_cover(page, cover)
        {
            report("could not change the page cover", err);
        }
        cx.notify();
    }
}
