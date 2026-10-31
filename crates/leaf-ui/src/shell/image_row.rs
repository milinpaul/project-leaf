//! Drawing an image block (E07-S08, ADR 0053): the picture from the workspace's asset store at the
//! width the owner gave it (or its natural size), shrunk to fit the column, a placeholder while it
//! loads, and a handle on its right edge to resize it. The sizes come from `leaf_editor`
//! (`image_display_size`, `resized_image_width`); the width is saved once, when the drag ends, as one
//! undo step.
//!
//! A picture is only read from disk when its row is drawn, and the virtual list draws only the rows on
//! screen, so a long page loads its pictures as they scroll into view.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, DragMoveEvent, EmptyView, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement, Pixels, Resource, SharedString, StatefulInteractiveElement as _, Styled, Window, canvas,
    div, img, px,
};
use gpui::{AppContext as _, ImgResourceLoader};
use gpui_component::{ActiveTheme as _, Icon, h_flex};
use gpui_kit_assets::IconName;
use leaf_core::{AssetId, BlockId};
use leaf_editor::{image_display_size, resized_image_width};

use super::{Shell, report};
use crate::theme;

/// The height of the frame shown while a picture loads, or when it can't be shown.
const FRAME_HEIGHT: f32 = 120.0;

/// The page column's side padding (`px_10` on each side), for the room an image has before its row
/// has been measured.
const COLUMN_PADDING: f32 = 80.0;

/// The value GPUI carries while an image's resize handle is dragged.
#[derive(Debug, Clone, Copy)]
pub(super) struct ResizingImage(pub(super) BlockId);

/// A resize in progress.
pub(super) struct ImageResize {
    block: BlockId,
    /// The drawn width when the drag began.
    start_width: f32,
    /// Where the handle was pressed. GPUI starts a drag only once the pointer has moved a few
    /// pixels, so this comes from the press, not the drag.
    start_x: Option<Pixels>,
    /// The room the image has (its column at its indent).
    available: f32,
    /// The width as of the latest move.
    width: f32,
}

/// How much room each image row had when last drawn: the width of the column at its indent.
#[derive(Clone, Default)]
pub(super) struct ImageRoom(Rc<RefCell<HashMap<BlockId, f32>>>);

impl ImageRoom {
    fn get(&self, block: BlockId) -> Option<f32> {
        self.0.borrow().get(&block).copied()
    }

    /// An invisible element filling the image's row that records its width, and asks for one more
    /// frame when it changed (a resized window), so the picture is sized for the new room.
    fn recorder(&self, block: BlockId) -> AnyElement {
        let room = self.clone();
        canvas(
            move |bounds, window, _| {
                let width = f32::from(bounds.size.width);
                let changed = room.0.borrow_mut().insert(block, width) != Some(width);
                if changed {
                    window.refresh();
                }
            },
            |_, (), _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }
}

impl Shell {
    /// The picture of an image row, filling `indent`-indented room in the column.
    pub(super) fn render_picture(
        &mut self,
        block: BlockId,
        asset: AssetId,
        stored: Option<f64>,
        indent: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = self
            .blocks
            .image_room
            .get(block)
            .unwrap_or(theme::READING_WIDTH - COLUMN_PADDING - indent);
        let recorder = self.blocks.image_room.recorder(block);
        let Some(storage) = self.storage.as_ref() else {
            return Self::frame(block, "Image not available in this window", None, cx);
        };
        let checked = self
            .blocks
            .drawable
            .entry(asset)
            .or_insert_with(|| storage.drawable_asset(asset));
        let path = match checked {
            Ok(path) => path.clone(),
            Err(problem) => return Self::frame(block, &problem.to_string(), Some(recorder), cx),
        };
        let resource = Resource::Path(Arc::from(path.as_path()));
        let natural = match window.use_asset::<ImgResourceLoader>(&resource, cx) {
            None => return Self::frame(block, "Loading image…", Some(recorder), cx),
            // Not reported: the failure is cached and this runs every frame. The frame says it.
            Some(Err(_)) => return Self::frame(block, "This image can't be shown", Some(recorder), cx),
            Some(Ok(data)) => {
                let size = data.size(0);
                #[expect(clippy::cast_precision_loss, reason = "image sizes are far below 2^24")]
                (size.width.0 as f32, size.height.0 as f32)
            }
        };
        let live = self
            .blocks
            .resizing
            .as_ref()
            .filter(|resize| resize.block == block)
            .map(|resize| f64::from(resize.width));
        let (width, height) = image_display_size(live.or(stored), natural, available);
        let group = SharedString::from(format!("image-{block}"));
        h_flex()
            .w_full()
            .relative()
            .child(recorder)
            .child(
                div()
                    .id(SharedString::from(format!("picture-{block}")))
                    .debug_selector(move || format!("picture-{block}"))
                    .group(group.clone())
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .child(img(path.clone()).size_full())
                    .child(Self::resize_handle(
                        block,
                        width,
                        available,
                        &group,
                        live.is_some(),
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// The bar on an image's right edge: shown on hover (and while dragging); drag it to resize.
    fn resize_handle(
        block: BlockId,
        width: f32,
        available: f32,
        group: &SharedString,
        dragging: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shell = cx.entity().downgrade();
        let bar = div().w(px(4.0)).h(px(40.0)).rounded_full().bg(cx.theme().border);
        div()
            .id(SharedString::from(format!("image-resize-{block}")))
            .debug_selector(move || format!("image-resize-{block}"))
            .absolute()
            .top_0()
            .right(px(-6.0))
            .h_full()
            .w(px(12.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_col_resize()
            .when(!dragging, |handle| {
                handle
                    .opacity(0.0)
                    .group_hover(group.clone(), |style| style.opacity(1.0))
            })
            .child(bar)
            .on_mouse_down(MouseButton::Left, {
                let shell = shell.clone();
                move |event, _, cx| {
                    let _ = shell.update(cx, |shell, _| shell.blocks.resize_press = Some(event.position.x));
                }
            })
            .on_drag(ResizingImage(block), move |_, _, _, cx| {
                let _ = shell.update(cx, |shell, _| {
                    shell.blocks.resizing = Some(ImageResize {
                        block,
                        start_width: width,
                        start_x: shell.blocks.resize_press.take(),
                        available,
                        width,
                    });
                });
                cx.new(|_| EmptyView)
            })
            .into_any_element()
    }

    /// The pointer moved while an image's handle is dragged: the image follows it.
    pub(super) fn on_image_resize_move(
        &mut self,
        event: &DragMoveEvent<ResizingImage>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dragged = event.drag(cx).0;
        let Some(resize) = self
            .blocks
            .resizing
            .as_mut()
            .filter(|resize| resize.block == dragged)
        else {
            return;
        };
        let x = event.event.position.x;
        let start = *resize.start_x.get_or_insert(x);
        let width = resized_image_width(resize.start_width, f32::from(x - start), resize.available);
        // Widths are whole pixels, so any change is at least one.
        if (width - resize.width).abs() >= 1.0 {
            resize.width = width;
            cx.notify();
        }
    }

    /// The handle was let go over the page: the new width is saved.
    pub(super) fn on_image_resize_drop(&mut self, _: &ResizingImage, _: &mut Window, cx: &mut Context<Self>) {
        self.finish_image_resize(cx);
    }

    /// Saves the width of a resize that ended. Called on drop, and at each frame while no drag is
    /// active, since a handle let go outside the page has no drop.
    pub(super) fn finish_image_resize(&mut self, cx: &mut Context<Self>) {
        if cx.has_active_drag() {
            return;
        }
        let Some(resize) = self.blocks.resizing.take() else {
            return;
        };
        if (resize.width - resize.start_width).abs() < 1.0 {
            return cx.notify();
        }
        let width = Some(f64::from(resize.width));
        if let Some(Err(err)) = self.page().map(|page| page.set_image_width(resize.block, width)) {
            report("could not resize the image", err);
        }
        cx.notify();
    }

    /// A frame where a picture would be: while it loads, or when it can't be shown.
    fn frame(
        block: BlockId,
        label: &str,
        recorder: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        h_flex()
            .id(SharedString::from(format!("image-frame-{block}")))
            .debug_selector(move || format!("image-frame-{block}"))
            .relative()
            .h(px(FRAME_HEIGHT))
            .w_full()
            .justify_center()
            .gap_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .text_color(muted)
            .children(recorder)
            .child(Icon::new(IconName::Image).text_color(muted))
            .child(label.to_owned())
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
