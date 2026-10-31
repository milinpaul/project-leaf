# ADR-066. Databases: one Loro document per database, rows as pages

- **Status:** Accepted (owner merged the spike with its answers, 2026-10-30)
- **Type:** Architecture
- **Date:** 2026-10-30
- **Story:** E09-S01

## Context

Phase 1 adds databases (`PRODUCT_SPEC.md` "Database (single view type: Table)"):
- property types: text, number, select, multi-select, date, checkbox, URL;
- add, rename, reorder and hide columns; sort and filter; inline cell editing;
- each row is a page you can open and write in;
- a database can be a full page or sit inline in a page.

The constraints already in place:
- documents are Loro CRDTs so two devices can merge (Phase 2 sync);
- only commands change documents, and undo is per document;
- unknown data from a newer version is never dropped;
- a page opens in under 100 ms.

The simple table (ADR-061) is not a database: no types, no sorting, and rows are not pages.

The spike (`crates/leaf-model/tests/database_spike/`) drove Loro 1.17 directly to try a model before any production
code exists.

## Decision (proposed)

**One Loro document per database**, stored like a page (`DocKey::Database(DatabaseId)`):

```
LoroDoc (database)
├── "columns": LoroTree (flat)     column order; node meta "id" = column ID
├── "schema":  LoroMap             column ID -> LoroMap { "name", "type", "options", "width", "hidden", ...unknown keys kept }
│                                  (a column exists while its schema entry does; see the correction below)
├── "rows":    LoroTree (flat)     row order; node meta "id" = the row's PageId, "deleted" once deleted
├── "cells":   LoroMap             row ID -> mergeable LoroMap: column ID -> value
└── "view":    LoroMap             sort, filters, hidden columns, widths (one table view for now)
```

- **Cells** are plain values, last writer wins per cell:
  - text and URL: string;
  - number: f64;
  - checkbox: bool;
  - date: i64 unix ms;
  - select: option ID.
- **Multi-select** is a mergeable map of option ID to `true`, so tags two people add are all kept.
  - *Amended by E09-S12 (2026-10-31):* once a cell has its map, the map is never taken out of the row. A mergeable
    map put back after being taken out is the same container, holding whatever it held then, and undo and redo
    don't follow edits made while it was out; so a cell retyped, cleared and made a multi-select again lost its
    options on undo. Now a plain value set after a retype is kept inside the map under the key `=`, and clearing
    the cell empties the map. An emptied map reads as no value, except in a multi-select, where it reads as no tags.
- **Select options** live in the column's schema (`options`: ordered list of `{id, name, color}`); cells hold option IDs,
  so renaming an option is one edit.
- **Rows are pages.**
  - Each row is a page in the workspace document: its title is the database's title column, and its body is an ordinary
    page document.
  - The database document holds only order and properties, so a table view never opens 10,000 page documents.
  - A row's page entry is marked as a row of its database, so the sidebar leaves it out (the owner's answer 1, below).
- **A full-page database** is a page whose content is a database. **An inline database** is a `database` block in a page
  that points at a database document; the block holds no data.
- **Cells whose row or column is gone stay in `cells`**, and readers ignore them:
  - undoing a column delete brings the column back with its values;
  - an edit that arrives after a delete is kept rather than lost.
- **Unknown property types** (from a newer version) show read-only. An older version writes only the keys it knows
  (name, order, width), so the column's settings and its cells pass through untouched.
- **Reading:** the view shows the first screen of rows straight away and finishes reading the rest in the background.
  It keeps a materialized copy updated from Loro's change events rather than re-reading every cell.

## Findings

All of these are tests in the spike (11 tests, plus one measurement).

| Question | Result |
|---|---|
| Schema, order and cells in one document, saved and loaded | Works; a column move is one move in the order. |
| Two peers set different cells of a row at once | Both kept, but only because the row's map is *mergeable* (`ensure_mergeable_map`). With an ordinary nested map, one peer's whole row would replace the other's. |
| Two peers set the same cell | One value wins, the same on both peers. |
| Rename a column while the other peer fills it | Both kept. |
| Delete a column while the other peer fills it | The column stays deleted; the late value is kept aside in `cells`. |
| Add tags to one multi-select cell on both peers | All kept; a tag removed on one peer goes. |
| Rows added on both peers | All kept. |
| **Row deleted on one peer, moved on the other: movable list** | **The deleted row comes back.** Both peers agree, but a deleted row reappears. The same happens to a column's place in the order (its schema stays deleted, so readers must skip order entries without a schema). |
| Row deleted on one peer, moved on the other: tree | Delete wins in the order the spike tried; **corrected by E09-S02: Loro's order of the two concurrent edits decides, so sometimes the move wins** (below). |
| Property type from a newer version, older peer renames and moves it | Its settings (`expression`, a nested `format` map) and its cells' values survive. |
| Undo a cell edit; undo a column delete | Both restore exactly the earlier schema and table. |

Measured with 10,000 rows × 8 columns (all seven types) in a release build:

| Measure | Result |
|---|---|
| Snapshot on disk | 2.1 MB |
| One cell edit | 131 bytes of update; 7 ms |
| Load the snapshot | 3 ms (Loro loads lazily) |
| Read the first screen (50 rows) | 14 ms |
| Read every cell | 180–200 ms, about 130 of them in Loro building the values |
| Sort by a number / filter by a select (already read) | 4 ms / 2–3 ms |
| Alternative: each row's properties in its own document | 6.8 MB on disk, 250–280 ms just to load the 10,000 documents |

At 10,000 rows, reading everything at open would break the 100 ms page budget. Reading the first screen first and the
rest in the background keeps opening well inside it. Sorting and filtering need the whole column, so they wait for the
background read on very large databases. Measuring a per-column layout ("cells" keyed by column first) is left to
E09-S02, if sorting a 10,000-row database while it opens turns out to matter.

## Alternatives considered

- **Properties in each row's page document:** a table view would load one document per row (3.2× the size on disk, and a
  quarter of a second at 10,000 rows before any sorting). Merging and undo of one cell are no better.
- **Rows as children in the workspace page tree, properties in the workspace document:** every database's cells would
  load with the workspace at startup, and one big database would slow the sidebar and every save.
- **Extending the simple table block:** blocks hold rich text, not typed values. Rows wouldn't be pages, and every page
  edit would carry the table's undo history.
- **SQLite tables for cells:** fast queries, but no CRDT merging for Phase 2 sync and no undo through Loro. It would also
  be a second source of truth beside the documents.
- **Movable lists for row and column order:** simpler, but a concurrent move brings a deleted row back (above).

## Consequences

- **New `DocKey::Database` and a `Database` model in `leaf-model`,** with commands like `PageDoc`'s:
  - add, rename, retype, move, hide and delete a column;
  - add, move and delete a row;
  - set a cell;
  - set and clear the sort and filters.
- **Invariants, proptest convergence and undo round-trips** as for pages.
- **Creating a row creates a page in the workspace document too,** so a row add touches two documents. Undo needs a
  rule for that, as trashing pages already has.
- **Search indexes row titles and text properties.** Markdown export writes a database as a table, and CSV export is
  natural.
- **The owner's answers (2026-10-30)** to the open questions:
  1. **Row pages show only in the table** (and in search), not in the sidebar.
  2. **Text properties are plain text**, last edit wins per cell. URLs have their own type.
  3. **Changing a column's type converts on display.** Values that fit show converted, and the rest show blank. The
     stored value is kept until the cell is edited, so changing the type back loses nothing.
  4. **Deleting an inline database block trashes the database** and its row pages with it; undo or restore brings them
     back.
  5. **Design and benchmark for 10,000 rows.**
- **Plain English:** each database is saved as one file-like document holding its columns, row order and every cell's
  value. Each row is also an ordinary page you can open and write in. Two devices can edit the same database offline,
  and their changes combine sensibly when they sync: different cells both stick, tags add up, and a deleted row or
  column stays deleted. A 10,000-row database is about 2 MB, and its first screen shows in a few hundredths of a
  second.

## Correction (2026-10-30, E09-S02)

The two-peer property test written for E09-S02 found a merged column that was still in the order but had no schema.
In a Loro tree, a delete and a concurrent move of the same node settle by Loro's fixed order of concurrent edits
(here, by device ID). When the move sorts after the delete, it brings the node back. The spike tried only the case
where the delete sorted last, so the finding above was half right.

So **existence is recorded apart from order**, in values a move never touches:
- a column exists while its schema entry does, and deleting a column deletes only that entry;
- a row exists until its node's meta has `deleted: true`;
- neither node is ever taken out of its tree.

Readers also take an ID that a merge left on two nodes once (the first in order). Re-adding a deleted row clears its
mark and moves it to the new place, keeping its cells. Trees still give order without a deleted row coming back as a
duplicate, which a movable list can't.

Each column's options list is created with the column, so undo and redo of adding an option act on a list that was
always there. Creating it lazily on first use broke redo. A concurrent move can bring back a deleted option the same
way; that is harmless (an extra option in the list) and left as it is.

The spike's tree test now runs both cases.

## Undo across the two documents (2026-10-30, E09-S03)

A row lives in two documents: its row in the database and its page in the workspace. Adding or deleting a row must be
one undo step, so the database document decides which rows exist and the row page's trash state follows it:
- the workspace writes for row pages (creating one, trashing or restoring it to match) go outside the workspace's undo
  (`Workspace::apply_unrecorded`);
- after any database change, undo or redo, `WorkspaceEditor::reconcile_rows` brings the row pages in line;
- restoring a row page from the Trash is the one way in from the other side: it restores the row too, where it was
  (`DatabaseCommand::RestoreRow`).

