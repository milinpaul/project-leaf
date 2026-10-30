# 0067. Formatting bar: a floating bar over selected text, drawn in the page's own overlay

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E07-S13

## Context

Formatting was keyboard-only. The owner asked for a Notion-style bar over selected text with "Turn into", Bold,
Italic, Strikethrough, Code and Link, for any text selection, in one block or across blocks. The "Turn into"
toolbar at the top of the page was removed earlier (ADR 0057); this is a different thing: it appears only where
the owner is already looking, and only while text is selected.

## Decision

- **The model is in `leaf-editor`** (`PageEditor::format_bar`): `None` unless there is a non-empty text selection.
  It returns the mark buttons with "on" states (from `active_marks`), the Turn into list (the block menu's kinds,
  the shared kind ticked), and the button's label (the shared kind, or "Turn into").
  - No mark buttons when no selected text can take marks (only code).
  - No Turn into when a selected block can't change kind (a divider, an image, a table cell, an unknown block), so
    a click never rewrites one.
- **Clicks take the keyboard's paths:** `toggle_mark`, the link popover (`open_link_popover`), and
  `BlockAction::TurnInto` for every touched block. Each is one undo step, and the selection is kept.
- **Placement:** just above where the selection starts, from last frame's painted layout (like the slash menu).
  If that is above the top of the page's view, it goes below the line where the selection ends. It snaps inside
  the window.
- **The Turn into list is drawn in the bar's own overlay, not as a popup menu.** A popup menu takes the keyboard,
  which unfocuses the page, hides the selection and closes the bar under it. Rows act on mouse-down; the list
  closes when the selection changes.
- **Hidden** while the page doesn't have the keyboard, while a selection is being dragged, and while the link
  popover, language picker, slash menu or block menu is open. Typing or Escape collapse the selection, which hides
  it too.

## Alternatives considered

- **gpui-component's dropdown for Turn into:** it focuses the menu, so the bar disappeared under it (tried first).
- **A bar only for single-block selections:** the owner chose any text selection; the editor's toggles already work
  across blocks.
- **Shortcut text in the editor model:** keys are bound in `leaf-ui` (`page_keys`), so the tooltips' keys live there
  too.

## Consequences

- Every formatting action now has a mouse path; the keyboard shortcuts are shown in tooltips.
- Another floating layer to keep out of the way of menus; the list of "busy" states is in `shell::format_bar`.
- Plain English: select some text and a small bar pops up above it to make it bold, italic, struck through, code
  or a link, or to turn the line into a heading, list, quote or code block.
