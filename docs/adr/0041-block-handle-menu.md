# 0041. The block menu is editor state; Move up/down move among siblings

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S04

## Context

Each block needs a ⋮⋮ handle that shows on hover and opens a menu of block actions (ADR 0028). The owner asked for
Move up and Move down in the menu as well, and an icon on every entry. The menu must also work from the keyboard.
gpui-component has a dropdown menu, but its open state is private to the widget, so the keyboard could not open it
and tests could not drive it without a window.

## Decision

- **The menu lives in `PageEditor`** (`block_menu.rs`), like the slash menu (ADR 0026). The editor keeps which block
  the menu is for, the highlighted entry and whether the "Turn into" list is open. Entries are worked out each time,
  so a greyed-out entry always matches what the page allows. Keys (`MenuKey`), hovers (`highlight`) and clicks
  (`choose`) all go through it, and choosing runs `block_action` as one undo step.
- **Move up / Move down** are new `BlockAction`s. They move the action's blocks (one block, or the selection) one
  place among their siblings with the existing `MoveBlocks` command, children included. At the first or last sibling,
  or when the selected blocks have different parents, nothing happens and the entry is greyed out. Moving never
  changes nesting; that is Tab / Shift+Tab.
- **The UI** (`shell/block_menu.rs`) draws a grip in the row's left gutter, shown while the pointer is over the row
  or while its menu is open. The menu floats over the page with Lucide icons per entry, takes the keyboard in its
  own key context (Up, Down, Left, Right, Enter, Escape), and closes on a click outside. `Cmd/Ctrl+/` opens it for
  the caret's block. When it closes after a choice or Escape, the page gets the keyboard back.

## Alternatives considered

- **gpui-component's `DropdownMenu`:** less code, but no way to open it from a shortcut or to test it headless.
- **Move up past the parent (out of a nested list), as some editors do:** surprising, and it changes nesting.

## Consequences

- Dragging the handle to reorder is still E05-S04 / E07-S05.
- Plain English: hover over a block and a ⋮⋮ grip appears to its left. Click it (or press Cmd+/) for a menu to change
  the block's type, copy it, move it up or down, or delete it. One Undo reverses any of them.
