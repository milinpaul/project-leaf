# 0062. Mouse-wheel steps glide; dragging a selection past the page's edge scrolls

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Stories:** E17-S01, E17-S02

## Context

The owner found scrolling rough compared with a web page, and could not select blocks below the visible part of
the page by dragging. The page is GPUI's virtual list (ADR 0035). Trackpads send exact pixel movements, which the
list follows and macOS adds momentum to. Mouse wheels send whole lines, which the list turns into an instant 20 px
jump per notch. Frames are well inside budget, so the jumps are the roughness, not slow drawing. Dragging blocks by
their handle already scrolled at the edges (ADR 0050); a text selection drag only heard the pointer over text.

## Decision

- **Wheel glide** (`shell/smooth_scroll.rs`): an invisible layer over the page takes wheel line steps in the capture
  phase, before the list, and glides the page there with an ease-out (time constant 40 ms, about 150 ms a notch)
  through `ListState::scroll_by`. One line is 40 px, Chrome's step. Steps add up, so fast notches stay smooth and
  exact. Pixel steps pass through untouched, and so does the wheel over a popover (the layer's hitbox must be
  hovered).
- **Edge scrolling while selecting** (`shell/select_scroll.rs`): while a text drag is in progress, a window-wide
  pointer listener scrolls near or past the top or bottom of the page on a 16 ms tick. Each tick it extends the
  selection to the text under the pointer, held inside the page, using the rows drawn last frame. Button up, or the
  pointer back away from the edges, stops it.
- **One speed rule for both drags:** `block_drag::scroll_speed` now keeps speeding up past the edge, to three times
  the edge speed two edge-widths out, for text and block drags alike.

## Alternatives considered

- **Patch GPUI's list to animate line deltas:** the right place long term, but a fork of GPUI is ruled out; the
  layer is small and can go if GPUI gains it.
- **Animate with the frame clock (`request_animation_frame`):** steps tied to frames, but the timer with the
  executor's clock is testable with the test clock and the ease uses the real time between ticks.
- **Scroll only while the pointer is past the edge:** a zone just inside the edge (48 px) matches block dragging and
  common editors.

## Consequences

- Wheel distance per notch doubles (40 px, as in browsers). Trackpad scrolling is unchanged.
- Plain English: turning a mouse wheel now slides the page smoothly instead of jumping, and dragging to select text
  down past the bottom (or up past the top) of the window scrolls the page and keeps selecting, faster the further
  out you go.
