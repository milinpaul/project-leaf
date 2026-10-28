# 0001. Loro CRDT as the document model

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E00-S03

## Context

Leaf is local-first today and will sync between devices and people later. Pages are trees of blocks with rich
text, and every edit must be undoable. Retrofitting sync onto a plain data model usually means rewriting it.

## Decision

Store every document in a [Loro](https://loro.dev) CRDT from day one: one workspace doc for the page tree and
one doc per page. `LoroTree` holds the block hierarchy, `LoroText` the rich text, and Loro's `UndoManager`
gives undo. Only `Command`s in `leaf-model` write to Loro. Details in
[BLOCK_MODEL.md](../architecture/BLOCK_MODEL.md).

## Alternatives considered

- **Plain Rust structs + our own undo:** simplest now, but sync later needs a CRDT or operational-transform rewrite anyway.
- **Yjs (`yrs`):** mature, but its tree support is weaker; block moves would need hand-rolled ordering.
- **Automerge:** good merge semantics, but slower for large texts and no native movable tree.

## Consequences

- Sync, merge and history come almost for free later; two-peer convergence is testable now with `proptest`.
- We depend on Loro's tree and text APIs staying stable; E00-S05 checks the features we rely on.
- Documents are stored as Loro snapshots plus updates, not readable files, so export matters (Phase 1).
- Plain English: every page is kept in a format designed to merge edits from several devices without conflicts,
  even though Leaf only runs on one device for now.
