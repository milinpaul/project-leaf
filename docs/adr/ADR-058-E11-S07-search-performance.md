# ADR-058. Search performance: rank only the newest matches, pick blocks before excerpts

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-30
- **Story:** E11-S07

## Context

E11-S07 sets search budgets on the full-size fixture (5,000 pages, 100,000 blocks, ADR-055): a query in under
50 ms at the 95th percentile, and re-indexing one edited page in under 20 ms. The first measurement, on a 4-core
2.1 GHz cloud VM, missed both: queries p95 67 ms, re-index p95 22 ms. The fixture's vocabulary is small, so every
word matches about 11,000 blocks: a hard but honest case (a common word, or the first letters typed).

Where the time went:
- **Ranking:** FTS5 scores every match with BM25, about 1.3 ms per thousand matches. "a" matched 35,000 blocks.
- **Excerpts:** made for up to 1,000 rows, while the bar shows at most 50 pages × 3 blocks.
- **One-letter prefixes:** the prefix indexes started at two letters, so "a" expanded every word starting with "a"
  on each pass.
- **Re-indexing:** each pass listed every searchable page with its title from the workspace document (about 12 of
  16 ms), although only one page's blocks had changed.

## Decision

- **Rank only the newest 10,000 matching blocks** (`RANKED_BLOCKS`). Below that every match is ranked as before.
  Above it, an older block can be left out; that only happens for words so common that 50 results can't show all
  their pages anyway. Titles are always all searched, and a page just edited is always among the newest.
- **Pick before excerpts.** The query ranks the best 2,000 candidates, keeps each page's best three, and makes
  excerpts for at most 300 rows, in one pass over the matches. This also fixes an older weakness: a page with
  hundreds of matching blocks could fill the row limit and hide every other page.
- **Prefix indexes from one letter** (`prefix = '1 2 3'`): the index file grows about 8%. `INDEX_VERSION` goes to
  3, so existing indexes are rebuilt once.
- **The indexer keeps the list of searchable pages** with the workspace version it was read at, and rebuilds it
  only when titles or the page tree change.
- **A benchmark,** `leaf-bench/benches/search.rs` (`just bench-search`), measures a full rebuild, 13 typical queries
  20 times each and 100 single-page re-indexes, and fails locally when a budget is missed. It has its own small
  harness because the budget is a 95th percentile, which divan doesn't report. CI runs it with the other benches on
  `main` and records the numbers without failing (`LEAF_BENCH_RECORD_ONLY`), as ADR-056 decided for noisy runners.

## Alternatives considered

- **Order very broad searches by date instead of relevance:** cheap, but a page with many matching blocks
  crowded out the rest, and fixing that needed a join over every match (about 60 ms).
- **Excerpts made in Rust from the stored text:** would need our own accent folding to match FTS5's; more code
  and a second source of truth for what matched.
- **Look up excerpts one row at a time:** FTS5 re-runs the whole match for each row; slower than everything else.
- **Fewer rows only (no per-page pick):** faster, but makes the crowding problem worse.

## Consequences

- On the same VM: queries p50 about 20 ms, p95 26–28 ms, worst about 39 ms; re-index one page p50 about 5 ms,
  p95 6–8 ms; full rebuild of 100,000 blocks about 8.8 s, on the indexer's own thread.
- Existing search indexes rebuild once after updating (a few seconds for a large workspace, in the background).
- Plain English: searching a very large workspace now answers in about a fiftieth of a second, and a page you just
  edited is searchable within a few milliseconds of being saved. For extremely common words, Leaf looks through
  the most recent ten thousand matching paragraphs rather than every one.
