# 0044. Search bar UI: one popup for Cmd/Ctrl+K and P, searched off the UI thread, results opened in place

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E11-S06

## Context

The spec gives one search box for `Cmd/Ctrl+K` (with no text selected) and `Cmd/Ctrl+P`, finding pages by title and
by their text. Results are grouped by page with excerpts, and choosing one opens the page at the matching block.
The model is `leaf_editor::SearchBar` (ADR 0042), and the index is `leaf-search`'s FTS5 file (ADR 0019). Searching
must never make typing stutter.

## Decision

- **One popup, `leaf_ui::search`.** It replaces the quick switcher's popup (E06-S04): Cmd/Ctrl+P now opens search,
  which matches titles the same way and adds full text. The `QuickSwitch` action became `Search`, and the File menu
  item "Go to Page…" became "Search…".
- **Cmd/Ctrl+K is `SearchOrLink`.**
  - The shell asks `shortcut_action` whether the page has selected text.
  - If not, it opens search.
  - If so, it is the link action, which does nothing until the link popover (E07-S07) exists. Opening search over
    a selection would surprise anyone used to Cmd+K for links.
- **Searching.**
  - A gpui timer fires at the bar's deadline, trusting the timer over the clock.
  - The query runs on the background executor through a read-only `Searcher`, opened per search (cheap, and it
    never waits for the indexer's writes).
  - The answer is converted to `SearchHit` and delivered with its id, so stale answers are dropped by the model.
  - A missing or broken index is logged once and gives no full-text results; titles still match.
- **Drawing.**
  - Page rows: icon, title (matched letters bold), breadcrumb.
  - Matching lines indented beneath, with the matched words bold on a soft accent tint.
  - "Searching…" while a search runs, "No results for “…”", and key hints at the foot.
  - Rows have two heights, so the list is a plain scrolled column, not a fixed-height virtual list. There are at
    most 50 pages with 3 lines each.
- **Opening in place: `Shell::open_at`.** It opens the page and, for a block, puts the caret at its start, scrolls
  it to the top of the view, and tints it with the accent colour fading out over 1.6 s.

## Alternatives considered

- **Keeping the quick switcher as a separate popup:** two boxes that both find pages, and Cmd/Ctrl+P would differ
  from the spec.
- **A long-lived `Searcher` shared with the background task:** it saves a file open per search, but needs a lock
  around a connection. Opening is fast next to the 150 ms pause.
- **Searching on the UI thread:** simplest, but a slow disk would freeze typing in the search field.

## Consequences

- Cmd/Ctrl+K over selected text does nothing until E07-S07 adds the link popover.
- Pages edited since the indexer last ran may match by title (live) but not yet by text. The indexer runs after
  each autosave, so this lasts about a second.
- Plain English: press Ctrl+K or Ctrl+P and type. Page names show at once, and after a moment so do the lines
  inside pages that contain your words, under their page. Pick one, and Leaf opens that page right at the line,
  which glows briefly so you can spot it.
