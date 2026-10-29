# 0020. Structural keys at the caret, one undo step per press

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E04-S05

## Context

`PRODUCT_SPEC.md` "Editor behaviour" fixes what Enter, Backspace, Delete and Tab do. The interim editor (E07-S10)
already had block-level helpers (`enter_at`, `backspace_at_start`, `indent`), called with a block and a position
chosen by the UI. Since E04-S02 the editor keeps its own caret and selection, so it can decide everything itself.

Some presses need more than one command. Enter over a selection deletes it, then splits. Leaving a code block trims
its empty lines, then adds a paragraph. One command is one undo step, so these would take two undos.

## Decision

- **`PageEditor::press(Key)`** handles Enter, Cmd/Ctrl+Enter, Backspace, Delete, Tab and Shift+Tab at the caret
  and selection, and returns `KeyOutcome`: where the caret is now, or a block to select. The UI only maps keys.
  - **Backspace / Delete:** inside text, remove one whole character (grapheme), or the selection.
  - **At a block's edge:** the spec's convert and merge rules. Delete at the end merges the next block in, with its
    children.
- **Code blocks:**
  - Enter inserts a newline.
  - Cmd/Ctrl+Enter leaves the block from anywhere.
  - Enter at the very end, after two empty lines, also leaves it. The two empty lines are removed and a paragraph
    follows.
- **Dividers and images** can't hold the caret:
  - Backspace just after one, or Delete just before one, returns `SelectBlock`. Nothing is deleted by accident.
  - `PageEditor::delete_block` deletes a selected block with its children. A page never ends up empty.
  - In an image caption, Enter adds a paragraph below instead of splitting the caption. Backspace at its start
    selects the image. Delete at its end does nothing.
- **One press, one undo step:** `PageDoc::as_one_step` wraps several commands in a Loro undo group. Each command
  is still its own commit, so saving and sync are unchanged. Undo returns the caret to where it was before the key.

## Alternatives considered

- **New compound commands in the model** ("replace and split", "exit code"): one commit each. But every key
  combination would need a model command, and the model would know about keys.
- **Rely on the undo merge interval:** quick commands merge anyway, but not reliably, and not in tests.
- **Delete a divider straight away on Backspace:** quicker, but a stray key would remove content without
  showing it first. Selecting it first matches the spec's Backspace rule, and Delete mirrors it.

## Consequences

- Block selection (drawing it, Escape, arrow keys across selected blocks) is E04-S06. Until then the UI calls
  `delete_block` for a `SelectBlock` outcome when the key is pressed again.
- Tab in a code block indents the block, like elsewhere. E07-S09 may change it to insert spaces.
- Plain English: Enter, Backspace, Delete and Tab now do what the product spec says, and any single key press can
  be undone with one Undo, with the cursor going back to where it was.
