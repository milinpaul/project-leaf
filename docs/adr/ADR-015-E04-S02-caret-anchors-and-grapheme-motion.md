# ADR-015. Carets anchored with Loro cursors, moved by grapheme

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E04-S02

## Context

The model counts text positions in Unicode scalars (`char`s). A reader sees fewer characters than that: an emoji
family joined with zero-width joiners, a flag, an accented letter written with a combining mark, or a Devanagari
conjunct are each one character on screen but several scalars. A caret that stops inside one splits it.

A caret stored as a plain number also goes stale. Text typed before it elsewhere (another device later, undo now)
should push it along. And undo can re-create a deleted block under a new Loro node and text container (ADR-006), so
anything tied to the old container is lost.

## Decision

- **Movement is by grapheme cluster and word (Unicode UAX #29)**, using the `unicode-segmentation` crate.
  - Left and Right move one grapheme; a caret placed mid-cluster snaps back to the cluster's start.
  - Word jumps work the macOS way: skip spaces and punctuation, then go to the end (or start) of the word.
  - Home and End stop at newlines. Wrapped lines are left to the layout (E04-S03).
  - At a block's edge, Left and Right cross into the neighbouring text block. Dividers and images are passed over.
- **The editor keeps the caret as a model `Anchor`:** a Loro cursor tied to a character, plus the caret it was
  made from.
  - `PageDoc::resolve` follows the cursor.
  - If the block's container changed (undo re-created it), resolving uses the stored position, clamped. The editor
    then re-anchors, so the caret follows the new container from there on.
  - If the block is gone, the caret goes to the end of the nearest text block above where it was.
- **Loro types stay in `leaf-model`.** `Anchor` wraps the cursor. `PageDoc::import_changes` merges another copy's
  changes; in Phase 0 only tests use it.

## Alternatives considered

- **Move by `char`:** simple, but splits emoji and Indic text.
- **ICU segmentation (`icu_segmenter`):** more complete, much heavier, with data files to ship.
- **Keep a plain position and shift it by hand after each command:** misses undo and remote changes, and repeats
  what Loro's cursors already do.
- **Store Loro cursors in undo steps:** they don't survive a re-created container. Undo steps keep `BlockId` +
  position (E02-S08).

## Consequences

- `unicode-segmentation` is a new direct dependency of `leaf-editor`. It is pure Rust, builds for wasm, and was
  already in the build through GPUI.
- Every caret position the editor hands out is on a grapheme boundary; a property test checks every motion.
- Plain English: the cursor jumps over a whole emoji or a whole Hindi syllable in one key press, and stays next to
  the same word when text before it changes, including after undo.
