//! Mouse-wheel scrolling that glides, as in a web browser (E17-S01).
//!
//! Trackpads send exact pixel movements, which the page list already follows (macOS adds the momentum).
//! Mouse wheels send whole lines, which GPUI's list turns into an instant jump per notch. Over the
//! page, this takes the line steps before the list sees them and glides the page there instead: a
//! short ease-out that adds up exactly to the distance, so many notches in a row stay smooth.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, DispatchPhase, HitboxBehavior, IntoElement as _, ScrollDelta, ScrollEvent,
    Styled as _, Task, canvas, px,
};

use super::Shell;

/// How far one wheel line scrolls, in pixels: what Chrome uses (GPUI's list uses 20).
pub(super) const LINE_PX: f32 = 40.0;

/// The ease's time constant: each `TAU` the distance left shrinks to about a third, so a notch is
/// done in about 150 ms.
const TAU: Duration = Duration::from_millis(40);

/// How often the glide moves the page.
const TICK: Duration = Duration::from_millis(8);

/// Below this many pixels left, the glide finishes in one step.
const SETTLE_PX: f32 = 0.5;

/// The distance still to glide: negative up, positive down.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct Glide {
    remaining: f32,
}

impl Glide {
    /// Adds `distance` pixels to go, on top of whatever is still gliding.
    pub(super) fn add(&mut self, distance: f32) {
        self.remaining += distance;
    }

    /// Whether there is nothing left to glide.
    pub(super) fn is_idle(self) -> bool {
        self.remaining == 0.0
    }

    /// How far to move after `elapsed`: the share of what is left that the ease covers in that
    /// time, or all of it once it is under half a pixel. The steps always add up to what was added.
    pub(super) fn step(&mut self, elapsed: Duration) -> f32 {
        let share = 1.0 - (-elapsed.as_secs_f32() / TAU.as_secs_f32()).exp();
        let mut moved = self.remaining * share;
        if (self.remaining - moved).abs() < SETTLE_PX {
            moved = self.remaining;
        }
        self.remaining -= moved;
        moved
    }
}

/// The glide in progress, and the task that plays it.
#[derive(Default)]
pub(super) struct WheelGlide {
    glide: Glide,
    last: Option<Instant>,
    task: Option<Task<()>>,
}

impl Shell {
    /// An invisible layer over the page that takes mouse-wheel line steps before the list does and
    /// glides them out. Pixel steps (trackpads) and the wheel over a popover are left alone.
    pub(super) fn wheel_glide_layer(cx: &mut Context<Self>) -> AnyElement {
        let shell = cx.entity().downgrade();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let shell = shell.clone();
                window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                    let ScrollDelta::Lines(lines) = event.delta else {
                        return;
                    };
                    // Shift+wheel scrolls a wide table sideways instead (E16-S03).
                    if phase != DispatchPhase::Capture
                        || !hitbox.should_handle_scroll(window)
                        || lines.y == 0.0
                        || event.modifiers.shift
                    {
                        return;
                    }
                    cx.stop_propagation();
                    // The wheel turned towards the user (negative lines) moves down the page.
                    let _ = shell.update(cx, |shell, cx| shell.glide_by(-lines.y * LINE_PX, cx));
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element()
    }

    /// Adds `distance` pixels to the glide and starts playing it if it isn't already.
    pub(super) fn glide_by(&mut self, distance: f32, cx: &mut Context<Self>) {
        let wheel = &mut self.blocks.wheel;
        wheel.glide.add(distance);
        if wheel.task.is_some() {
            return;
        }
        wheel.last = Some(cx.background_executor().now());
        wheel.task = Some(cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let going = shell.update(cx, Self::glide_tick);
                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        }));
    }

    /// One step of the glide. Returns whether to keep going.
    fn glide_tick(&mut self, cx: &mut Context<Self>) -> bool {
        let now = cx.background_executor().now();
        let wheel = &mut self.blocks.wheel;
        let elapsed = wheel
            .last
            .map_or(TICK, |last| now.saturating_duration_since(last));
        wheel.last = Some(now);
        let moved = wheel.glide.step(elapsed);
        if wheel.glide.is_idle() {
            wheel.task = None;
        }
        let going = wheel.task.is_some();
        self.blocks.list.state.scroll_by(px(moved));
        cx.notify();
        going
    }
}

#[cfg(test)]
mod tests;
