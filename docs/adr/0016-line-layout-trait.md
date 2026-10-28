# 0016. Vertical caret movement through a layout trait

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E04-S03

## Context

Up and Down move the caret between *visual* lines. Where a paragraph wraps depends on the font, the window width and
indentation, which only the UI knows after it has laid out the text. The rules for where the caret goes, though, are
editor behaviour. `PRODUCT_SPEC.md` says: at the first or last line, move to the previous or next block and keep the
horizontal position. Architecture rule 3 puts that behaviour in `leaf-editor`, which cannot depend on GPUI.

## Decision

- **`leaf-editor` defines a `LineLayout` trait** that the UI implements from its text layout:
  - `line_count(block)`;
  - `line_of(block, offset)` (at a wrap point, the later line);
  - `offset_to_x(block, offset)`;
  - `x_to_offset(block, line, x)`.
- **Units:** offsets are Unicode scalars, like the model. x is in page coordinates shared by every block, so a nested
  block's indentation counts and the caret stays in the same screen column.
- **`PageEditor::move_vertically(Up | Down, &layout)` holds the rules:**
  - within a block, go to the line above or below;
  - at its edge, go to the last or first line of the neighbouring text block (dividers and images are passed over,
    as with Left/Right);
  - at the top of the page, Up goes to the start; at the bottom, Down goes to the end.
- **Remembered x:** the caret keeps a preferred x while Up/Down repeat, so a short line in between doesn't pull it
  left for good. Any other caret move forgets it.
- **Testing:** a fake monospace layout (wraps at a fixed width, one column per grapheme, indents by depth) stands in
  for the UI.

## Alternatives considered

- **Compute wrapping in `leaf-editor`:** it would need font metrics, so it would duplicate the UI's text shaping and
  drift from what is drawn.
- **Handle Up/Down entirely in `leaf-ui`:** puts editor rules in the UI, untestable without a window.
- **Pass the layout as data (a list of line boxes per block):** works too, but the UI would have to build it for
  every block on each key press. A trait lets it answer only the blocks asked about.

## Consequences

- `leaf-ui` must implement `LineLayout` from GPUI's text layout when the real text element lands (E07-S02). Until
  then the interim text inputs handle Up/Down themselves.
- Plain English: the ↑ and ↓ keys go to the line above or below, even inside a wrapped paragraph, and the cursor stays
  in the same column as it passes short lines.
