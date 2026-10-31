# ADR-057. No "Turn into" toolbar: block types change from the slash menu, the block handle and Markdown

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E07-S11

## Context

The interim editor (E07-S10) put a row of buttons above every page (Text, H1, H2, H3, • List, 1. List, To-do,
Quote, Code), because it was then the only way to change a block's type with the mouse. Since then:
- the slash menu (E07-S06) converts a block, or adds one, from `/`;
- the block handle's menu has a "Turn into" list with the same kinds (E07-S04);
- Markdown shortcuts convert as you type (E05-S01).

The toolbar takes the top of every page, sits above page covers (E06-S11), and isn't part of the Notion-like look
in `PRODUCT_SPEC.md`.

## Decision

- Remove the toolbar. The page header (icon, cover, title) is now the first thing on a page.
- Keep `Shell::turn_focused_into`. It changes the block holding the caret through the same `BlockAction::TurnInto`
  the block menu uses, for tests and any future caller.
- A test checks that the header starts at the top of the page.

## Alternatives considered

- **Keep it, hidden until hover or focus:** still a second, older way to do what three newer ones do, and it would
  pop in over the cover.
- **A floating formatting toolbar over selected text (as in Notion):** that's for inline marks, not block types. It
  can come later as its own story if wanted.

## Consequences

- Nothing is lost: the toolbar only changed the block holding the caret, which the block menu also does.
- Mouse users find block types through the ⋮⋮ handle or by typing `/`.
- Plain English: the row of Text / H1 / H2 / … buttons at the top of each page is gone. To change what a line is, type
  `/` or use the ⋮⋮ handle beside it, as in Notion.
