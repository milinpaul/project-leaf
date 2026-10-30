# 0061. Simple tables: rows and cells as blocks inside a table block

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-30
- **Story:** E16-S01

## Context

The owner added a simple table to Phase 0 (E16): a plain grid of rich-text cells with an optional header row, so a
pasted Markdown table becomes a table instead of loose lines. It is not a database (E09, Phase 1): no column
types, sorting or filtering, and rows are not pages. The model must give it commands with exact undo, let two
copies merge (for sync later), and keep data from newer versions.

The choice is the shape in the Loro document:

- **A. Blocks all the way down:** a `table` block whose children are `table_row` blocks whose children are
  `table_cell` blocks, each cell with its own text container.
- **B. One block holding a grid:** a `table` block with a list of rows, each a list of text containers.

## Decision

**A.** Rows and cells are blocks with `BlockId`s, and a cell is a text block.

- Everything that already works on a block works on a cell: the text and mark commands, the caret and selection
  (a block and a position), undo and redo, merging, saving, search indexing, the incremental view. The editor's
  table work (E16-S02, S03) reuses all of it.
- **Columns are positions within rows**, without IDs. Column commands change one cell in every row.
- **Guards keep the grid whole.** Rows and cells change only through the table commands (insert, delete and move a
  row or column; the header row is a property of the table). The general block commands refuse to delete, move,
  duplicate, convert, indent, split or merge a row or cell on its own, and can't put anything else inside a table.
  Whole tables move, delete and duplicate like any block.
- **Merges may leave rows uneven.** If one copy adds a column while another adds a row, the new row is one cell
  short. That is allowed: a missing cell reads as empty, and the next table command evens the rows out as part of
  the user's change. The merge repair (ADR 0044) does not pad rows: it would create cells on both copies with
  different IDs, and the copies would no longer match. The repair only puts misplaced blocks back in order (a
  stray block leaves the table; a stray cell becomes a paragraph), so nothing is lost.
- **Newer versions' data is kept:** extra keys on a table, row or cell, and unknown marks in cells, are never
  rewritten. A new invariant 9 says rows sit in tables and cells in rows.

## Alternatives considered

- **B, one block with a grid:** a table would be a leaf block, so no guards would be needed, but every text
  command, the caret, undo bookkeeping, merging and search would need a second "cell inside a block" address. Much
  more new code for the same result.
- **Column IDs** (a list of columns on the table, each cell naming its column): concurrent column moves and inserts
  would merge more exactly. Not needed while Phase 0 has no sync; can be added later as extra data without changing
  this shape.
- **Repairing uneven rows after a merge:** would make copies differ (see above).

## Consequences

- The general block commands now check for table parts; a new error `TablePart` names the row or cell.
- Until the table UI (E16-S03) the editor draws a table as a read-only placeholder row, copying leaves it out
  (pasting comes in E16-S04), and Markdown copy writes each cell's text on its own line.
- Plain English: a table is stored like a small stack of blocks (table, rows, cells), so typing in a cell, undo
  and saving all work the way they already do for paragraphs. Rows and columns can only be changed with the table's
  own commands, which keeps it a proper grid.
