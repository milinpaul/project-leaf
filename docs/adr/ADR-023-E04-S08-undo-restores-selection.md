# ADR-023. Undo restores the selection before an edit; redo the one right after

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E04-S08

## Context

Since E02-S08 each undo step stored one caret, the one set before the command. On undo the editor got it back.
On redo it got the caret from when undo was pressed. Two gaps:

- A selection, possibly across blocks or of whole blocks (E04-S06), came back as just a caret.
- Redo put the caret where the user happened to be when they undid, not where the edit had left it.

Loro's undo manager carries one value per step and gives it to `on_pop` when the step is undone or redone. There is
no way to change a step's value after it is pushed, and a step can't know its "after" when it is pushed.

## Decision

- **`SavedSelection { caret, tail, blocks }`** in `leaf-model` describes the editor's selection: the caret, the
  other end of selected text, and selected whole blocks.
- **Each step stores a number**, not a caret. `PageDoc` keeps a table: step number → selection *before* and *after*.
  - The number survives undo/redo: Loro calls `on_pop` before the `on_push` of the inverse step, so the push reuses
    the popped number.
  - Steps merged by the merge interval or an undo group keep the first number. So a burst of typing keeps the
    first "before" and the last "after".
- **The editor records both selections.** Every editing action calls `set_selection` (its "before") and
  `record_after` (its "after").
  - `record_after` only touches the top step if the page changed since the paired `set_selection`. A key that did
    nothing can't overwrite an older step's "after".
- **`History::Done { selection }`:** undo returns the step's "before", redo its "after". If no "after" was recorded
  (an edit made outside the editor), redo returns the "before".
- **`PageEditor::undo`/`redo`** restore it: caret, the selection's other end, and any selected blocks. They also end
  an input-method composition.

## Alternatives considered

- **Encode the selection itself in Loro's step value:** works for "before", but the "after" isn't known when the step
  is pushed, and Loro has no way to update a pushed value.
- **Loro's own cursor list on each step:** stores positions, but not "after" either, and not whole-block selections.
- **Keep "redo returns the caret from undo time":** simpler, but redo would move the caret somewhere the edit never
  left it.

## Consequences

- The table grows by one entry per commit for the page's open life (merged commits leave unused entries). These are a
  few bytes each and dropped when the page closes; history isn't saved.
- Callers outside the editor that apply commands directly get no recorded selection; undo then returns none.
- Plain English: Undo now brings back exactly what was selected before the change, even a selection across several
  blocks or whole selected blocks. Redo puts the cursor where the change left it, even if you moved it in between.
