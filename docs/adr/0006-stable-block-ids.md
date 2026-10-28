# 0006. Stable block IDs stored in block meta

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E02-S10

## Context

BLOCK_MODEL.md defined `BlockId` as Loro's `TreeID`. The E00-S05 spike (ADR 0001) showed that when undo or
redo re-creates a block (undo of a delete, redo of a create), Loro gives it a **new** `TreeID`, and a different
one on every cycle. Content, children, order and meta all come back. Several things need a block identity
that never changes:
- restoring the cursor and selection after undo;
- Phase 1 links to a block, stored as `(PageId, BlockId)`;
- comparing documents in invariant 7.

## Decision

Every block's meta holds a permanent `id`: a UUID v7, stored as a string. This is the same pattern pages already
use (`PageId` in the workspace doc's page meta).

- `BlockId` is a UUID newtype in `leaf-core`, like `PageId`. It does not depend on Loro.
- `TreeID` is an internal handle that never leaves `leaf-model`. The model keeps a `BlockId → TreeID` index,
  and rebuilds it after every undo, redo and import, which are the only times Loro re-creates nodes.
- New blocks get a fresh `BlockId` when created. Pasted and duplicated blocks always get fresh ones, so IDs
  stay unique within a page.

## Alternatives considered

- **Keep `BlockId = TreeID` and remap on undo:** listen to undo events and patch every stored reference.
  Fragile: links in other pages would need rewriting, and every new feature that stores a block reference would
  have to remember to.
- **Soft delete (never delete tree nodes, mark them deleted):** fixes undo of a delete, but not redo of a
  create, and deleted nodes pile up in the document forever.
- **Our own undo with inverse commands:** full control over IDs, but we would lose Loro's undo, which already
  handles edits from other peers correctly. That is a large, risky rewrite.
- **A shorter ID (random `u64`):** about half the size, but UUID v7 matches `PageId`, is unique across pages
  for copy and paste, and sorts by creation time.

## Consequences

- Block identity survives undo, redo, moves and sync. Cursor restore and block links can rely on it.
- Each block stores roughly 36 extra bytes, before Loro's compression. That's acceptable at 10,000 blocks.
- Rebuilding the index after undo or redo is a scan of the page's tree. That's cheap at 10,000 blocks; if
  profiling (E08) disagrees, update the index from Loro's events instead.
- Invariant 7 compares documents by `BlockId`, ignoring `TreeID`s. New invariant: every block has a unique `id`.
- Plain English: every block gets its own permanent name tag, so undo and links can always find the same block,
  even when the underlying library gives it a new internal number.
