# 0044. Pages are repaired after a merge, the same way on every copy

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E02-S09

## Context

E02-S09's property test has two copies of a page edit at the same time and swap changes. In 10,000 random runs the
copies always end up identical, as a CRDT promises. But a merge can break the page's rules (`BLOCK_MODEL.md`
invariants) even though each copy only ran commands that keep them:

- one copy turns a block into a heading or code while the other nests a block under it (invariant 3);
- one turns a block into code while the other bolds a word in it (invariant 5);
- both change a block's kind at once: the merged kind can lack its text container (2) or a heading its level (4).

The owner chose repairing such pages over tolerating them (2026-10-29), so the rest of Leaf can keep trusting the
invariants.

## Decision

- **`PageDoc::import_changes` repairs the page after every merge** (`page_doc/repair.rs`). Repairs follow what a
  local conversion already does, and drop nothing the user wrote:
  - children step out of a block that can't hold them, right after it, in order;
  - code text loses its marks;
  - a text block without a text container gets an empty one; a divider holding words becomes a paragraph;
  - a heading without a level gets level 1; an image without its file becomes a paragraph keeping its caption.
- Every copy sees the same merged page, so each makes the same repairs, and repairs merge cleanly with each other.
  The property test checks this: after every swap both copies are identical and every invariant holds.
- **Repairs are not undoable.** They are committed with their own origin, which the undo manager excludes, so Undo
  reverses the user's own edits and never "un-repairs" the page into a broken state.
- Blocks of unknown kinds are left alone: their rules belong to a newer version.

## Alternatives considered

- **Tolerate broken merged pages:** relax invariants 2–5 for merged pages and make every reader cope. Less code now,
  but every later feature (editor, export, search) would have to handle pages that "can't exist".
- **Reject or reorder conflicting operations:** not possible with a CRDT; every copy must accept every change.

## Consequences

- Each repair is a small local write, so two copies that repair at once briefly produce two equal edits; they merge
  to the same page.
- Phase 0 has no sync, so this only runs in tests today; it is in place when sync arrives.
- Plain English: if two people edit the same block in conflicting ways at the same moment (one makes it a code
  block, the other indents something under it), Leaf tidies the result the same way on both computers. Nothing
  typed is lost, and both see the same page.
