//! Drag-selecting text past the top or bottom of the page scrolls it, faster the further out the
//! pointer goes, and keeps extending the selection while it scrolls, even with the pointer still
//! (E17-S02). Dragging blocks by their handle does the same in `block_drag`, at the same speeds.
//!
//! A drag over the text itself is followed by `rich_text`; this listens to the pointer anywhere in the
//! window, because past the page's edge it is over no block at all.

use gpui::{
    AnyElement, Context, DispatchPhase, IntoElement as _, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Styled as _, Task, canvas, point, px,
};
use leaf_model::Caret;

use super::Shell;
use super::block_drag::{SCROLL_TICK, scroll_speed};

/// The scrolling while a text selection is dragged near an edge.
#[derive(Default)]
pub(super) struct SelectScroll {
    /// The pointer, in window coordinates.
    pointer: Option<Point<Pixels>>,
    /// Pixels per tick: negative up, positive down, 0 away from the edges.
    speed: f32,
    /// Scrolls while `speed` isn't 0.
    task: Option<Task<()>>,
}

impl Shell {
    /// An invisible layer that follows the pointer anywhere in the window while text is being
    /// drag-selected, and stops the scrolling when the button comes up.
    pub(super) fn select_scroll_layer(cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        canvas(
            |_, _, _| {},
            move |_, (), window, _| {
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        let pressed = event.pressed_button == Some(MouseButton::Left);
                        let _ = moved.update(cx, |shell, cx| {
                            shell.on_select_pointer(event.position, pressed, cx);
                        });
                    }
                });
                let released = shell.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        let _ = released.update(cx, |shell, _| shell.end_select_scroll());
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    /// The pointer moved: while a selection is being dragged, scroll near or past the page's edges.
    fn on_select_pointer(&mut self, at: Point<Pixels>, pressed: bool, cx: &mut Context<Self>) {
        if !pressed || self.blocks.drag.is_none() {
            self.end_select_scroll();
            return;
        }
        let view = self.blocks.list.state.viewport_bounds();
        let scroll = &mut self.blocks.select_scroll;
        scroll.pointer = Some(at);
        scroll.speed = scroll_speed(f32::from(at.y), f32::from(view.top()), f32::from(view.bottom()));
        if scroll.speed != 0.0 && scroll.task.is_none() {
            scroll.task = Some(cx.spawn(async move |shell, cx| {
                loop {
                    cx.background_executor().timer(SCROLL_TICK).await;
                    let going = shell.update(cx, Self::select_scroll_tick);
                    if !matches!(going, Ok(true)) {
                        break;
                    }
                }
            }));
        }
    }

    /// The button came up, or the drag ended: stop scrolling.
    pub(super) fn end_select_scroll(&mut self) {
        self.blocks.select_scroll = SelectScroll::default();
    }

    /// One step: scroll, then stretch the selection to the text under the pointer, held inside the
    /// page. Returns whether to keep going.
    fn select_scroll_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let scroll = &self.blocks.select_scroll;
        let (Some(pointer), speed) = (scroll.pointer, scroll.speed) else {
            return false;
        };
        if speed == 0.0 || self.blocks.drag.is_none() {
            self.blocks.select_scroll.task = None;
            return false;
        }
        self.blocks.list.state.scroll_by(px(speed));
        self.extend_selection_to(pointer);
        cx.notify();
        true
    }

    /// Stretches the drag's selection to the text at `pointer`, moved inside the page first, as the
    /// rows were drawn in the last frame.
    fn extend_selection_to(&mut self, pointer: Point<Pixels>) {
        let Some(from) = self.blocks.drag else { return };
        let view = self.blocks.list.state.viewport_bounds();
        let inside = point(
            pointer.x,
            pointer.y.clamp(view.top() + px(1.0), view.bottom() - px(1.0)),
        );
        let painted = &self.blocks.painted;
        let Some(block) = self
            .blocks
            .drawn
            .row_at(inside.y, |block| painted.contains(block))
        else {
            return;
        };
        let Some(at) = painted.offset_at(block, inside) else {
            return;
        };
        if let Some(page) = self.page() {
            let _ = page.select_between(from, Caret { block, at });
        }
    }
}

#[cfg(test)]
mod tests;
