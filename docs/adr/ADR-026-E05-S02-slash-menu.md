# ADR-026. Slash menu: the query is page text; a small built-in fuzzy filter

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S02

## Context

`PRODUCT_SPEC.md`: "Slash menu (`/`): insert or convert blocks, fuzzy filtered." The menu has to:
- open on `/` at the start of a word;
- filter as the user types;
- move with Up/Down;
- apply with Enter or a click;
- close on Escape or a space that matches nothing.

Applying removes what was typed. It converts an empty block, or inserts a new block below.

## Decision

- **The `/` and query are ordinary typed text in the page.** They show while the menu is open and stay if it
  closes. The editor keeps only an anchor on the `/` and the highlighted entry. Each time it is asked, it reads the
  query from the text between the `/` and the caret.
  - The menu closes by itself when the `/` is gone or the caret leaves the query.
  - It can't disagree with the page, including after undo or another device's edit.
- **Opens** after typing that ends in `/` at the start of a word: block start or after whitespace. Not in code blocks
  or image captions, whose text is literal.
- **Filter:** a dozen entries with a name and keywords, matched by a small function here rather than a fuzzy-search
  crate:
  1. name starts with the query;
  2. a word in the name, or a keyword, starts with it;
  3. the query's letters appear in order, fewer gaps first.

  Ties keep menu order. The query is trimmed and case doesn't matter.
- **Keys:**
  - `slash_move(Up/Down)` wraps, and a new query highlights the top entry again.
  - `press(Enter)` applies while something matches; with no match it is a normal Enter.
  - `slash_close()` handles Escape.
  - A space with no match closes the menu.
- **Apply** (`slash_apply`): removes `/query`.
  - An empty block is converted.
  - Otherwise the new block goes right below and takes the caret.
  - A divider never takes the caret: the caret stays put, or goes to a new paragraph when the divider replaced an
    empty block.
  - It is an undo step of its own (`as_separate_step`, ADR-025), so undo right after gives back the typed `/query`.

## Alternatives considered

- **A fuzzy-matching crate (`nucleo`, `fuzzy-matcher`):** better on long lists of file names, but a new dependency
  for ten fixed entries, with scores that are hard to pin down in tests.
- **Keep the query in the editor, not the page:** nothing would need deleting on apply. But the typed text would
  vanish if the menu closed, and it wouldn't show on screen without the UI drawing it separately.

## Consequences

- An image entry comes with E05-S07, when there is a file picker. The popover itself, positioned at `SlashMenu::at`,
  is E07-S06.
- Plain English: typing `/` opens a list of block types that narrows as you type. Enter or a click picks one: it
  turns an empty line into that block, or adds it below. Escape, or a space when nothing matches, closes the list and
  leaves what you typed.
