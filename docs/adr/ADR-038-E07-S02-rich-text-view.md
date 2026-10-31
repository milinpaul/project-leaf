# ADR-038. Block text drawn with GPUI's StyledText; line layout read back from the painted text

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E07-S02

## Context

Blocks were drawn as plain text inputs, which can't show formatting: links, bold and the rest were saved but invisible.
E07-S02 asks for:
- text drawn with its marks;
- the editor's caret (blinking) and selection, across blocks;
- a `LineLayout` (ADR-016) that matches the painted text, so Up and Down move between real wrapped lines.

Typing into such an element (keys, input methods, mouse) is E07-S03. The owner chose to land the two separately and
keep the app editable after each merge.

## Decision

- **Text:** each block's text is a `StyledText` with one `TextRun` per piece of equal formatting.
  - Bold and italic set the font.
  - Inline code uses the monospace face on a tint.
  - Links get the link colour and an underline; strikethrough is drawn.
  - A whole block's look (heading size and weight, monospace code, a done to-do muted and struck) sets the base.
  - One-line rows keep the inputs' 28 px height, so nothing shifts when a block switches between the two.
- **Selection:** a background colour on the selected pieces, so it sits under the glyphs and follows wrapping exactly.
  - A text selection covers its two ends in part and the blocks between whole.
  - Selected blocks cover everything nested in them.
  - The theme's selection colour arrives translucent and is too faint on the warm page, so it is drawn at full
    strength.
- **Caret and layout:** an overlay painted after the text records the measured layout and draws the caret, 2 px wide
  in the theme's caret colour. It blinks every 530 ms. The timer runs only while the text view draws a caret.
- **`PaintedLayout`** implements `LineLayout` from the layouts recorded in the last frame:
  - visual lines are the distinct tops of character positions;
  - x is the window x, shared by every block;
  - "closest to x on a line" picks the nearest drawn boundary;
  - blocks not on screen answer as one line;
  - every boundary's position is worked out in one pass over GPUI's glyphs, once per frame, the first time it is
    needed (amended in E17-S08). Asking GPUI one boundary at a time walked the line from its start on every call,
    so a click in a 38,000-character pasted paragraph took over a minute.
- **Interim, until E07-S03:** the block being typed in still shows its text input. Clicking any other block switches
  it to the input with the cursor where the click landed (`PaintedLayout::offset_at`).
- **Screenshot tool:** gains `caret=B:C` and `select=B:C-B:C`, through `Shell::editor_mut`, to check this visually.

## Alternatives considered

- **gpui-component's rich `TextView` or Markdown views:** made for read-only documents, with their own model. Our
  runs, selection and caret come from `PageEditor` and need byte-exact control.
- **A fully custom `Element` with its own text shaping:** more control, but `StyledText` already shapes, wraps and
  answers position queries. The overlay adds what's missing.
- **Drawing the selection as quads over the text:** would cover the glyphs and need its own wrapping arithmetic. Run
  backgrounds get both right for free.

## Consequences

- E07-S03 routes typing, input methods and the mouse into this view, drops the inputs, and wires Up/Down to
  `PaintedLayout`.
- Plain English: formatting is now visible on the page. Links show in colour and underlined, and bold, italic,
  strikethrough and inline code look as they should. The editor can draw its own cursor and selection, including
  across several blocks.
