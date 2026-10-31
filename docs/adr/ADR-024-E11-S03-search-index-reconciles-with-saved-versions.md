# ADR-024. The search index reconciles with saved versions on a background thread

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E11-S03, E11-S04

## Context

The search index (ADR-019) must reflect every saved edit within a second, never slow typing, and survive
crashes, renames, trash and permanent deletes. `leaf-search` may depend only on `leaf-core` and `leaf-model`, so
it can't call `leaf-store` directly. Search must never put user data at risk.

## Decision

- **Compare, don't report.** The editor doesn't tell search what changed. The store reports each document's
  saved version (`Store::saved_versions`: last save time, and the highest update sequence number, which grows
  with every save so two saves in one millisecond still differ). The index stores the version it indexed each
  page at. A *reconcile* re-reads pages whose version or title changed and drops pages that are trashed or gone.
  A rebuild is "empty the index, then reconcile".
- **Only saved data.** The indexer reads documents through a `PageSource` trait over a second store connection
  (`Workspace::reader`), never the editor's in-memory state, so the index can always be rebuilt from disk.
- **Own thread.** `SearchWorker` owns the index on a background thread. The window only *pokes* it after autosave
  writes something (a channel send); bursts of pokes become one reconcile. Quitting stops a rebuild between pages
  (it reruns next time) but still applies the last pokes.
- **Repair, don't fail.** Any index error is logged; the index is reset (tables dropped and recreated in place,
  so it works while the search bar has the file open on Windows; the file is replaced only if that fails) and
  rebuilt. A persistent failure waits for the next save instead of spinning.
- **Searching** uses its own read-only connection (`Searcher`); WAL lets it read while the worker writes. Typed
  text is always quoted before it reaches FTS5, so no input can break or redirect the query.

## Alternatives considered

- **The editor reports changed pages after each save:** less work per save, but every new way of changing a page
  (rename, trash, undo, a future sync) must remember to report, and edits saved just before a crash would be
  missed for good.
- **Index on the UI thread:** simplest, but a rebuild of thousands of pages would freeze the window.
- **Compare save times only:** two saves can share a millisecond, so a change could be missed.

## Consequences

- Each reconcile reads two small lists (saved versions, indexed pages); unchanged pages cost nothing more. The
  workspace document is re-read only when it changed.
- `leaf-store` gains `saved_versions` and `Workspace::reader`; `leaf-ui` gains a tiny `StoreSource` adapter.
- The index schema is versioned (`INDEX_VERSION` 2); changing it just triggers a rebuild.
- Plain English: whenever Leaf saves, a helper in the background checks which pages changed and updates its search
  copy of just those, so typing is never slowed down, and if the copy is ever damaged or out of date Leaf quietly
  rebuilds it from your saved pages.
