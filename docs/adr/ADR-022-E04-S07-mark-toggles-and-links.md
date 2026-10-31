# ADR-022. Mark toggles decided over the whole selection; caret toggles wait for typing

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E04-S07

## Context

`PRODUCT_SPEC.md` lists the inline shortcuts:
- `Cmd/Ctrl+B/I/E`: bold, italic, inline code;
- `Cmd/Ctrl+Shift+S`: strikethrough;
- `Cmd/Ctrl+K` with text selected: link.

The model already has `SetMark` and `RemoveMark`. The editor has to decide what a toggle means:
- when the selection is partly formatted;
- when it spans blocks, some of which are code;
- when nothing is selected.

## Decision

- **Toggle over a selection** (text in one or many blocks, or selected whole blocks): if *every* character already
  has the mark, remove it; otherwise apply it to all of them. One undo step (`PageDoc::as_one_step`). The selection
  stays.
  - Code blocks take no marks. They are left out both when deciding and when applying, so selecting across a code
    block still toggles the text around it.
- **Toggle with just a caret:** nothing is written. The editor remembers the toggle for the next typed text.
  - Typing applies it to the new characters, in the same undo step as the typing, and keeps it while typing carries
    on. This matters for inline code, which by the mark table doesn't grow on its own.
  - Any caret move forgets it.
- **`active_marks()`**, for the toolbar's state:
  - with a selection: the marks every selected character shares;
  - with just a caret: what the next typed text will get (the expand rules, then any pending toggles).
- **Links:**
  - `set_link(href)` links the selection, or changes the address of the whole link the caret is in. A blank
    address removes the link.
  - `remove_link()` unlinks the selection, or the whole link at the caret.
  - `link_at_caret()` gives the link's extent and address for the popover (E07-S07). The caret counts as in a link
    when a linked character touches it on either side.
  - Toggling `Link` only removes, and only when everything selected is linked: a link can't be added without an
    address.

## Alternatives considered

- **Decide the toggle by the first selected character** (some editors do): quicker, but a partly bold selection
  could flip either way depending on where the drag started.
- **Write the toggled mark immediately at the caret** (a zero-width mark): Loro marks need a range, and an empty
  mark would sit in the document and sync to other devices for nothing.
- **Insert the link's address as text when nothing is selected:** the spec gives `Cmd/Ctrl+K` without a selection
  to search, so there is no such case.

## Consequences

- The keyboard bindings and the link popover live in the UI (E07-S03, E07-S07); they call these methods.
- Plain English: the formatting shortcuts work like in other editors. On a partly bold selection, Cmd+B first makes
  it all bold, and pressing it again removes the bold. With nothing selected it formats what you type next. Links
  can be added, changed and removed, and one Undo reverts each change.
