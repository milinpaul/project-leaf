# ADR-XXX. Searching databases, and their speed budgets

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-11-01
- **Story:** E09-S10

## Context

ADR-066 (answer 5) asked for databases to be searchable and measured: search should find rows by their titles and
their text properties, and `PRODUCT_SPEC.md` should state budgets for a 10,000-row database. Rows are pages, so their
titles were already indexed; their cells live in the database document, which changes without its row pages
changing. The owner moved database export to the Markdown import/export epic (E12-S01) and chose the budgets on
2026-11-01.

## Decision

- **Cells are indexed under their row's page.** Each text or URL cell of a row whose page isn't in the trash is a
  `blocks` row of kind `property` under the row's page, with `block_id` `<database>/<column>`. A hit on one lists the
  row and opens it at the top, where its properties are (`BlockHit::block` is `None`). Numbers, dates, checkboxes and
  options aren't indexed.
- **A database is tracked like a page.** `PageSource` reports each saved database's version and loads databases;
  the index (version 6) keeps a `databases` table of the version it read. When a database's version changes, its
  cells are read again and only the entries whose text changed are written, so a one-cell edit in a 10,000-row
  database writes one entry. A row page's own re-index leaves its cells alone; a trashed row page takes them with it;
  a database whose page is gone is forgotten.
- **Budgets** for a 10,000-row database, in `PRODUCT_SPEC.md`: open to the first screen < 100 ms; every row read (in
  the background) < 1 s; a sort or filter change < 50 ms; editing one cell < 16 ms. `leaf-bench`'s `budgets` bench
  measures each (`database_open`, `database_read_all`, `database_sort`, `database_filter`, `database_edit_cell`),
  on every `main` run in CI, which records the numbers (shared runners are too noisy to fail on, ADR-056).
- **A fix found by the benchmark.** Opening took ~105 ms: building the database's ID index checked each row node
  against the list of root nodes, a quadratic scan over 10,000 rows. A set makes it linear; opening now takes ~70 ms.

## Alternatives considered

- **A separate FTS table for cells.** Cleaner to tell apart, but the search query, ranking and excerpts would all need
  a second path; a `kind` on the existing table reuses them.
- **Re-index every row of a database on each save.** Simpler, but a typed character in a 10,000-row database would
  rewrite 10,000 entries in the background.
- **Index cells under the database page.** A hit would open the table, not the row that matched.

## Consequences

- An index of version 5 is rebuilt once when Leaf opens a workspace after this change.
- Budgets measured on the CI container: open 71 ms, read all 143 ms, sort 6.6 ms, filter 6.3 ms, edit 5.6 ms
  (medians); the acceptance pass checks them on a laptop.
- Plain English: search now finds database rows by the words in their text and web-address columns, and opening a
  big table got faster. The product spec says how fast a 10,000-row table must be.
