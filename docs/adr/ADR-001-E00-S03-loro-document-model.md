# ADR-001. Loro CRDT as the document model

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

## Spike findings (E00-S05, `loro` 1.17.1)

Proved by `crates/leaf-model/tests/loro_spike.rs`:

- **Tree:** with `enable_fractional_index`, `create_at`, `mov_after` and `mov_to` keep sibling order, and a
  moved block keeps its meta and text. Fractional indexes sort in sibling order.
- **Marks:** `config_text_style` with the BLOCK_MODEL.md table behaves as specified. Bold grows when typing
  after it, inline code and links grow on neither side, and `unmark` clears part of a range.
- **Undo:** one `UndoManager` undoes tree and text edits together, and each undo restores the exact earlier
  state. Commits within the merge interval form one undo step, which is how typing will be grouped.
- **Sync:** two peers exchanging `ExportMode::updates` converge to identical state. Concurrent moves that
  would form a cycle (x under y on one peer, y under x on the other) resolve without one.
- **⚠ Block identity:** when undo or redo re-creates a block (undo of a delete, redo of a create), Loro gives
  it a **new `TreeID`**. Content, order and children come back, and so does anything in the block's meta.
  So `BlockId = TreeID` (BLOCK_MODEL.md) is not stable across undo, and "undo restores the exact state" holds
  only if TreeIDs are ignored. A stable ID stored in each block's meta survives every cycle. Decided in
  [ADR-006](ADR-006-E02-S10-stable-block-ids.md): stable IDs live in block meta.
- **Mergeable child containers:** create a block's `text` with `LoroMap::ensure_mergeable_text`.
  `get_or_create_container` is deprecated, because two peers creating the same key concurrently would each get
  their own container, and one text would be lost.
- **API notes:** export returns `LoroEncodeError`, a different error type from `LoroError`, so `leaf-model`
  needs to map both into its own error type. Positions in `insert`/`mark` are Unicode scalar offsets, as
  BLOCK_MODEL.md assumes.

## Consequences

- Sync, merge and history come almost for free later; two-peer convergence is testable now with `proptest`.
- We depend on Loro's tree and text APIs staying stable; E00-S05 checks the features we rely on.
- Documents are stored as Loro snapshots plus updates, not readable files, so export matters (Phase 1).
- Plain English: every page is kept in a format designed to merge edits from several devices without conflicts,
  even though Leaf only runs on one device for now.
