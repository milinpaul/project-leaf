# 0010. Documents track their own unsaved changes

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E03-S03

## Context

The store saves each document as a snapshot plus small updates (ADR 0003). Something has to know which changes
are already on disk, so a save writes only what is new and nothing when nothing changed. Loro types must stay
inside `leaf-model`, and `leaf-store` must not depend on how documents are built.

## Decision

- **Tracking in the model:** `Workspace` and `PageDoc` each remember the Loro version they were last saved at.
  - `unsaved_changes()` exports one update covering every change since then (commands, undo and redo alike), or
    `None` when nothing changed.
  - `mark_saved()` moves the mark forward. The store calls it only after the write succeeded, so a failed write
    is retried with the same changes.
  - `from_saved(snapshot, updates)` loads a document; the loaded state counts as saved.
- **The store:**
  - It handles any `Document` (a small trait over those four methods), and only ever sees bytes.
  - A document's first save writes its snapshot; every later save appends one row to `doc_updates`.
  - Rows are keyed by `DocKey`: `workspace`, or a page's `PageId`.
- **Time from the caller:** the caller passes the time in, so autosave can be tested with a fake clock.

## Alternatives considered

- **Subscribe to Loro's local-update events and queue the bytes:** one row per commit (no batching), and a
  failed write would need its own retry queue.
- **The store keeps version vectors per document:** Loro types would leak into `leaf-store`.
- **Always write a full snapshot:** simple, but every save would cost as much as the whole page.

## Consequences

- A save writes about as much as the edit itself, and nothing when idle.
- Autosave (E03-S04) only has to decide when to call `save`.
- Compaction (E03-S05) can rewrite the snapshot without touching how changes are tracked.
- Plain English: each page keeps a bookmark of what is already saved. Saving writes only what came after the
  bookmark, then moves the bookmark.
