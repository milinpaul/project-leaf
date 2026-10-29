# 0049. Dragging blocks: drops land in a gap between rows, at a depth picked by the pointer's x

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S04

## Context

The ⋮⋮ handle drags blocks to reorder or nest them (PRODUCT_SPEC, "Block handle"). The rules for where a drop lands
must be plain, tested editor logic (rule 3), so the UI only reports where it drew rows and where the pointer is, and
draws the line it is told to (E07-S05). A block must never land inside itself, and dragging a selection must keep the
blocks' order.

## Decision

- **A drop is a gap plus a depth** (`BlockDrop`), the way outliners work. The pointer's height picks the gap: the
  nearer edge of the row under it, or the nearest row when it is above or below them all. Its distance from where
  top-level text starts picks the depth, one level per indent width. The depth is clamped to what the gap allows:
  - at least the depth of the row below the gap, so the drop sits before that row in reading order;
  - at most one deeper than the row above, or the same depth when that block can't hold children (heading, code,
    divider, image, unknown kinds).
- **What the line shows is what happens.** One level deeper than the row above means "first child of that block".
  Shallower means "right after the row above's ancestor at that depth". So under a block with children, the drop
  goes above its first child, because that is where the line is.
- **The dragged blocks are taken out first.** The outline is read without them and everything inside them, so a gap
  inside the dragged blocks doesn't exist. A pointer over their own rows gives the gap where they already are, and
  dropping there does nothing. `drop_blocks` also refuses a parent inside the dragged blocks.
- **What is dragged** is `action_targets` for the grabbed handle: the selection when the handle's block is in it,
  else that block. `drop_blocks` moves the outermost of them with one `MoveBlocks` (which keeps their order) as one
  undo step. A drop that leaves them where they are, or refers to a block that has gone, changes nothing.
- The UI passes `RowBox`es, the top and bottom of the rows it drew, so only visible rows are needed; depths come from
  the page view.

## Alternatives considered

- **Before / After / Inside zones per row, like the sidebar (ADR 0031):** fine for pages, but in a page "inside" by
  height can't reach "after the parent" from under its last child, and the line can't show the depth. Using the
  pointer's x matches Notion and other outliners.
- **Pixel geometry in the editor (indent widths, padding):** the editor would have to mirror the UI's layout. Instead
  the UI passes row edges and the indent width it uses.

## Consequences

- Every rule is tested without a window, including a property test that every drop lands exactly where its line
  is, with nothing lost and the page's rules kept.
- E07-S05 only has to collect row edges, call `drop_target` on pointer moves, draw the line at `below` and `depth`,
  and call `drop_blocks` on release.
- Plain English: when you drag a block, Leaf shows a line where it will go. Moving up or down picks between which two
  blocks; moving right or left picks how deeply it is tucked in. A block can't be dropped inside itself, and a dragged
  selection stays in order. One undo puts everything back.
