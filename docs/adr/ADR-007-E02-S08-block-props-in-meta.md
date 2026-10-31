# ADR-007. Block properties stored directly in block meta

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E02-S08

## Context

BLOCK_MODEL.md stored a block's type-specific properties (heading level, to-do checked, code language, image
asset/width/alt) in a nested `props` map inside the block's meta. The page undo property test (E02-S08) found a
case where that loses data:

1. insert a paragraph (step 1);
2. turn it into a heading, which creates the `props` map and writes `level` (step 2);
3. undo both, then redo both.

Redoing step 1 re-creates the block under a new `TreeID` (ADR-006). Redoing step 2 then recreates the `props` map
**without** its `level`, leaving a heading with no level, which is invalid data. Loro restores values written to the
block's own meta correctly in the same situation, and nested maps that already existed when the block was created
are fine too. It is specifically a nested container created in a later step than its block. This looks like an
upstream Loro issue in how redo remaps re-created child containers.

## Decision

Store properties as plain values **directly in the block's meta**, next to `id`, `type` and `text`: `level`,
`checked`, `language`, `asset`, `width`, `alt`. There is no nested `props` map.

## Alternatives considered

- **Create the `props` map when every block is created**, so it never appears in a later step. That fixes blocks
  made in this session, but not blocks loaded from older data without a map, which would hit the same bug the first
  time their kind changes.
- **Our own undo with inverse commands:** full control, but a large rewrite and loses Loro's handling of edits from
  other peers (rejected in ADR-006 for the same reason).
- **Wait for an upstream fix:** leaves invalid data reachable today.

## Consequences

- Undo and redo restore properties exactly; the property test checks this for random sequences of every command
  (10,000 cases in CI on `main`).
- Simpler code: no nested container to create, read or clean up. Stale properties of a previous kind are removed
  from the meta when the kind changes; keys written by newer versions are kept.
- Property names share the meta with `id`, `type` and `text`, so those three names are reserved and can't be used
  as properties.
- No migration needed: nothing is saved to disk yet (E03).
- Plain English: each block's settings (like a heading's size or a to-do's tick) are now stored right on the block,
  which fixes a case where redo could forget them.
