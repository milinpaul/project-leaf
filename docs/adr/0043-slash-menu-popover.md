# 0043. The slash popover keeps the page's keyboard and opens above the line near the bottom edge

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S06

## Context

The slash menu (ADR 0026) has lived in `PageEditor` since E05-S02: it filters as you type, and Up, Down, Enter and
Escape already drive it. Nothing drew it. It needs a popover by the `/` that works with the keyboard and the mouse
and stays usable near the window's edges.

## Decision

- **The page keeps the keyboard.** Unlike the block menu (ADR 0041), the popover takes no focus: the query is typed
  text, so typing must keep reaching the page. The editor's existing keys move the highlight and choose.
- **The mouse:** hovering an entry highlights it (new `PageEditor::slash_highlight`), clicking applies it; the click
  doesn't reach the text underneath, so the caret stays in the query.
- **Placement** (`slash_popover::place`): below the line when the list fits, otherwise above it, otherwise on the
  roomier side. Its height is known in advance (fixed row height), so the choice is made before layout. Sideways
  it is nudged inside the window. GPUI's own "flip when it doesn't fit" would cover the line being typed, so it
  isn't used.
- It is placed by where the `/` was drawn in the previous frame: this frame's layout is only known after painting.

## Alternatives considered

- **Give the popover focus and forward typed text to the page:** more moving parts for no gain.
- **Snap to the window only:** near the bottom edge it would slide up over the line being typed.

## Consequences

- If the line rewraps as you type, the popover catches up on the next frame (the caret's blink redraws it).
- Plain English: type `/` and a list of block types appears just under it (or just above, near the bottom of the
  window). Keep typing to narrow it, use the arrow keys and Enter, or click one.
