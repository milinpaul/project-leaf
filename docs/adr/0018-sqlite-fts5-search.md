# 0018. Full-text search with SQLite FTS5 in a disposable file, opened with Ctrl+K

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E11 (epic; stories E11-S01 to E11-S07)

## Context

The owner moved full-text search across the workspace from Phase 1 into Phase 0, with a Notion-style
`Cmd/Ctrl+K` search bar. `TECH_STACK.md` had planned `tantivy`. A personal workspace is thousands of pages
and around 100k blocks, and the target is results in under 50 ms on 5,000 pages. The index must never hold
data that exists nowhere else, and Phase 0 has no network access. `Cmd/Ctrl+K` was already the link shortcut.

## Decision

- **Engine:** SQLite FTS5, which is compiled into the SQLite that `rusqlite` already bundles. No new
  dependency. Ranking uses FTS5's BM25 with a boost for title matches.
- **Where the index lives:** its own file, `search/index.sqlite`, never inside `workspace.sqlite`. It is
  disposable: if it is missing, damaged or built by an older index version, it is deleted and rebuilt from the
  pages in the background. It needs no schema migrations, only an index version number.
- **What is indexed:** one row per text block (page, block, block kind, text) plus one row per page title.
  Pages in the trash are left out. Blocks of unknown kinds are indexed by their text if they have any.
- **Keeping it current:** after autosave writes a page, that page is re-indexed off the UI thread. The index
  only reads documents; it never changes them.
- **Boundary:** `leaf-search` owns the index behind a small API (`index_page`, `remove_page`, `rebuild`,
  `search`). Nothing else touches FTS5 directly, so the engine can be swapped later.
- **Shortcut:** `Cmd/Ctrl+K` opens search when no text is selected and edits a link when text is selected
  (as in Notion). `Cmd/Ctrl+P` always opens search. The search bar is the quick switcher (E06-S04) extended
  with full-text results.

## Alternatives considered

- **tantivy:** better typo tolerance and scale, but a large dependency tree, slower builds, a bigger binary
  and a second storage system, for a scale a personal workspace does not reach.
- **FTS5 tables inside `workspace.sqlite`:** one file, but then the index needs migrations, grows the user's
  real data file, and can't simply be deleted and rebuilt.
- **A separate shortcut for search (only `Cmd/Ctrl+P`):** no conflict, but not what Notion users expect.

## Consequences

- No new dependency; `leaf-search` gains `rusqlite`. It stays native-only, so a future web client needs its
  own search backend.
- Typo-tolerant ("fuzzy") full-text matching is limited to prefix matching; titles still get fuzzy matching
  from the quick switcher.
- The `Cmd/Ctrl+K` handler must check for a text selection first.
- Plain English: search keeps its own throwaway copy of the text of every page, so finding a word is
  instant, and if that copy is ever lost Leaf quietly rebuilds it from your pages.
