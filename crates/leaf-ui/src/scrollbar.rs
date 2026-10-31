//! Leaf's scrollbar (E07-S14): one slim look for every long area, so the page and the sidebar tell you where you
//! are without taking attention. A rounded pill with no track, always faintly there so the position can be read at a
//! glance, a little wider and darker under the pointer and while dragged. Colours come from the theme (ADR-027), so
//! it follows light and dark.
//!
//! It is gpui-component's `Scrollbar`, styled; scrolling by wheel, trackpad, drag and click are its own.

use gpui::{Hsla, px};
use gpui_component::ActiveTheme as _;
use gpui_component::scroll::{Scrollbar, ScrollbarHandle, ScrollbarMode};

/// Thumb width at rest, and under the pointer or while dragged.
const THUMB: f32 = 4.0;
const THUMB_ACTIVE: f32 = 7.0;
/// Gap between the thumb and the area's edge.
const INSET: f32 = 3.0;
/// The shortest the thumb gets on a very long page, so it stays easy to grab.
const MIN_LENGTH: f32 = 36.0;

/// How strongly the thumb shows: faint at rest, clearer on hover, clearest while dragged.
const REST: f32 = 0.28;
const HOVER: f32 = 0.5;
const DRAG: f32 = 0.65;

/// The vertical scrollbar for a scrolling area, laid over its right edge. Put it as the last child of the area's
/// `relative()` container.
pub fn slim<H: ScrollbarHandle + Clone>(handle: &H, cx: &gpui::App) -> Scrollbar {
    let ink = cx.theme().muted_foreground;
    let shade = |opacity: f32| -> Hsla { ink.opacity(opacity) };
    Scrollbar::vertical(handle)
        .mode(ScrollbarMode::Always)
        .styles(|styles| {
            styles
                .track(|track| {
                    track
                        .bg(gpui::transparent_black())
                        .border_color(gpui::transparent_black())
                })
                .track_hover(|track| {
                    track
                        .bg(gpui::transparent_black())
                        .border_color(gpui::transparent_black())
                })
                .track_active(|track| {
                    track
                        .bg(gpui::transparent_black())
                        .border_color(gpui::transparent_black())
                })
                .thumb(|thumb| {
                    thumb
                        .bg(shade(REST))
                        .width(px(THUMB))
                        .inset(px(INSET))
                        .radius(px(THUMB / 2.0))
                        .min_length(px(MIN_LENGTH))
                })
                .thumb_hover(|thumb| {
                    thumb
                        .bg(shade(HOVER))
                        .width(px(THUMB_ACTIVE))
                        .inset(px(INSET))
                        .radius(px(THUMB_ACTIVE / 2.0))
                        .min_length(px(MIN_LENGTH))
                })
                .thumb_active(|thumb| {
                    thumb
                        .bg(shade(DRAG))
                        .width(px(THUMB_ACTIVE))
                        .inset(px(INSET))
                        .radius(px(THUMB_ACTIVE / 2.0))
                        .min_length(px(MIN_LENGTH))
                })
        })
}
