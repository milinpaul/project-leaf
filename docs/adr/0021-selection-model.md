# 0021. Selection: text between two anchors, or whole blocks

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E04-S06

## Context

Until now a selection lived in one block: the caret plus a *tail* anchor in the same block (E04-S04).
`PRODUCT_SPEC.md` asks for more:
- selecting within a block and across blocks (Shift+arrows, mouse drag);
- `Cmd/Ctrl+A` once for the block's text, twice for all blocks;
- typing over a cross-block selection as one undo step.

E04-S05 also needs a way to really select a divider or image.

## Decision

- **Two kinds of selection:** `Selection::Text { start, end }` and `Selection::Blocks(ids)`.
- **Text selection:** the caret plus a tail anchor that may be in another block. `start`/`end` are worked out in
  reading order whenever asked, so the selection follows edits, undo and other devices' changes. If the tail's
  block disappears, the selection ends.
- **Moving the selection:**
  - `extend(Motion)` and `extend_vertically` move the caret and keep the tail.
  - Plain Left/Right with text selected go to the selection's start/end.
  - Any other caret move ends the selection.
- **Block selection** is a mode: a list of blocks, each with what is nested in it. It is entered by:
  - `select_all` pressed a second time (or the first time in an empty block, which has no text to select first);
  - `select_blocks`, used by Backspace/Delete next to a divider or image.

  In the mode:
  - Backspace/Delete delete the blocks, as one undo step;
  - Tab/Shift+Tab nest them and keep them selected;
  - typing and Enter do nothing.

  Any caret move ends it.
- **Deleting text across blocks** (`delete_span`, in `leaf-editor`, built from existing commands inside
  `PageDoc::as_one_step`):
  - The end of the first block and the start of the last go.
  - Blocks in between go, with what is nested in them.
  - The last block's remainder joins the first.
  - Blocks that *contain* the last block can't be deleted, since that would delete it too. Their text is emptied
    and they are merged away, so their other children move up.
  - An image caption never takes in other text, so it isn't joined.
- **Over a cross-block selection, as one undo step each:**
  - typing, an input-method commit, Backspace and Delete replace it;
  - Enter deletes it and then splits;
  - Tab nests every block it touches.
- **Composition across blocks:** the input-method draft over a cross-block selection is drawn where the selection
  starts.

## Alternatives considered

- **A model command `DeleteRange`:** one commit instead of several. But it duplicates the merge and delete commands'
  rules, which are already tested, including with property tests. The undo group gives the user the same single step.
- **Store selection as two plain positions:** breaks as soon as anything edits text before it.
- **Block selection as text selection over whole blocks:** can't represent a selected divider, which holds no text.

## Consequences

- The UI (E07-S02) draws `Selection::Text` as a highlight per block and `Selection::Blocks` as selected rows.
- Undo/redo restoring the selection, not just the caret, is E04-S08.
- Plain English: you can now select text across several blocks with Shift and the arrow keys, or everything with
  Cmd/Ctrl+A pressed twice. Typing, Delete or Enter over that selection works in one step that a single Undo
  reverses.
