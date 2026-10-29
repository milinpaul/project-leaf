# 0042. Search bar model: titles at once, full text after a pause, stale results dropped by id

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E11-S05

## Context

The search bar (`Cmd/Ctrl+K` with no text selected, or `Cmd/Ctrl+P`) finds pages by title and by the text inside
them, grouped by page with title matches first (`PRODUCT_SPEC.md`, ADR 0019). Its behaviour must be plain, tested
Rust in `leaf-editor`. That crate is wasm-ok and may not depend on `leaf-search` (SQLite), so it can't run a search
itself. The index is queried off the UI thread, so results arrive late and possibly out of order.

## Decision

- **`leaf_editor::SearchBar`** builds on the quick switcher's snapshot of live pages (ADR 0033): the empty query
  lists recent pages, and typed titles are matched at once, letters in order.
- **Full text comes in as plain data.**
  - `SearchHit` / `BlockMatch` / `Marked` mirror the index's results.
  - `leaf-ui` converts `leaf_search::PageHit` into them (E11-S06), so `leaf-editor` gains no dependency.
  - The index marks characters; the model converts them to byte ranges for drawing.
- **A pause before searching, clock-free.**
  - Each keystroke moves the deadline to `now + SEARCH_DELAY_MS` (150 ms, a debounce).
  - `next_deadline()` tells the app when to set its timer, and `due(now)` hands over a `SearchRequest { id, query }`
    once.
  - Time is passed in, as with `Autosave`.
- **Stale results are dropped by id.** Every query gets a rising id. `deliver(id, hits)` is ignored unless `id` is
  the current query's, so a slow search for "gar" can never replace the results for "garden".
- **Rows are grouped by page.**
  - Order: pages whose title the index matched, then the switcher's title matches, then pages that matched only in
    their text.
  - Each page is followed by its matching blocks, and every row can be highlighted.
  - A page row whose title didn't match opens at its best block; a block row opens at that block.
  - Pages in the trash or no longer in the tree are dropped.
  - When results arrive, the highlight stays on the same row if it is still there.
- **Keys:** Up/Down wrap, Enter returns `Open(Destination { page, block })`, and Escape returns `Close`.
- **Shortcut decision:** `shortcut_action(Shortcut::ModK, text_selected)` is `Link` over selected text and
  `OpenSearch` otherwise. `ModP` always opens search.

## Alternatives considered

- **`leaf-editor` calling the index through a trait:** it would still need the call to be asynchronous, and the
  ordering problem would just move into the trait. Plain data in and out is simpler to test.
- **A throttle (search at most every N ms, as autosave does):** it searches half-typed words while typing goes on.
  A debounce waits for the word, and title matches already give instant feedback.
- **Comparing query text to spot stale results:** typing "ab", "a", "ab" would accept the first "ab" search's late
  results for the second. Ids can't repeat.

## Consequences

- E11-S06 only has to draw `rows()`, run `due()` requests on a background task and pass results back.
- Title matches show before the index answers, so the list may grow a little when full-text results arrive. The
  highlight is kept on the same row so it doesn't jump.
- Plain English: as you type in the search box, page titles appear straight away. When you pause, Leaf also looks
  inside pages and adds those results under each page; answers to an older search are thrown away.
