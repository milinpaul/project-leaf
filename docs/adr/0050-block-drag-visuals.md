# 0050. Block drag visuals: GPUI's drag, a line at the drop's depth, faded rows, scrolling at the edges

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S05

## Context

E05-S04 (ADR 0049) worked out where dragged blocks land. The page now needs the drag itself:
- the handle must still open its menu on a click (E07-S04);
- a preview follows the pointer;
- a line shows the gap and depth, so before, after and nested are told apart;
- holding the pointer near the top or bottom edge scrolls the page, which is a virtual list where only the rows on
  screen exist.

## Decision

- **GPUI's built-in drag** (`on_drag`, `on_drag_move`, `on_drop`), as the sidebar does (ADR 0031). GPUI starts a drag
  only after the pointer moves a few pixels, so the handle opens the menu with `on_click` (on release) instead of on
  press. The dragged value is the grabbed block; when the drag starts, the shell asks the editor for `action_targets`
  (the selection when the block is in it) and closes the menu.
- **Rows report where they were drawn.** Each row has an invisible `canvas` that records its bounds in the frame
  (`DrawnRows`). On every drag move, the shell passes them, the pointer and the indent width to `drop_target`. Only
  rows on screen are needed.
- **The line** is 3 px in the theme's ring colour (the sidebar's drop colour). It sits under the row the drop goes
  below, or above the first row, and is indented to the drop's depth. The dragged blocks and everything inside them
  are drawn at 40% opacity. The preview is a small solid popover with the first block's first line, and "+N" when
  more blocks come along.
- **Auto-scroll:** within 48 px of the page's top or bottom edge, a timer scrolls the list every 16 ms, faster closer
  to the edge (up to 18 px a tick), and works the drop out again as rows pass under the pointer. It stops away from the
  edges and when the drag ends.
- Letting go off the page moves nothing. The drag is forgotten on the next frame once GPUI has no active drag.
- The screenshot tool gets `drag=B:T[:L]` to show a drag in progress.

## Alternatives considered

- **Hand-rolled mouse down/move/up on the handle:** it would duplicate GPUI's drag threshold, preview and drop routing,
  and the sidebar already uses GPUI's.
- **Asking the list for item bounds (`ListState::bounds_for_item`):** these give the full-width item, not where the
  text column starts, which nesting depth is measured from. The recorder gives both.

## Consequences

- Window tests drive real drags: a drop above the first block, nesting by moving right, letting go off the page, a
  click still opening the menu, and holding near the bottom edge scrolling a 120-block page.
- Plain English: grab the ⋮⋮ handle and move it. A small label follows the pointer, the blocks you are moving go pale,
  and a green line shows where they will land. The line is indented when they will be tucked inside the block above.
  Hold near the top or bottom of the page to scroll. A plain click still opens the menu.
