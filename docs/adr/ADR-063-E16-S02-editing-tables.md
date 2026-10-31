# ADR-063. Editing tables: cells as text blocks, a rectangle of cells, tables acted on whole

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E16-S02

## Context

A table's cells are text blocks (ADR-061), so typing, marks, the caret, Left and Right, and undo already work in
them. What remained were the keys and selections that mean something else in a grid, and every editor feature that
might issue a command a cell refuses (split, merge, convert, move), which would have surfaced as an error.

## Decision

- **View:** the table's row carries `BlockStyle::Table { header_row, rows, columns }`; its cells follow it in the
  rows, row by row, one level deeper, as `BlockStyle::TableCell { row, column, header }`. Row blocks aren't listed.
  The UI builds the grid from these.
- **Keys in a cell:** Tab / Shift+Tab move to the next / previous cell (caret at the end of its text, like Notion);
  Tab in the last cell adds a row. Enter types a line break. Backspace at a cell's start and Delete at its end do
  nothing, except that Backspace in a table whose cells are all empty deletes it. Up/Down keep to the column and leave
  the table past its top or bottom row; coming in, they pick the cell under the caret's x (from the layout the UI
  already gives Up/Down).
- **Selection:** a text selection from one cell into another of the same table is `Selection::Cells`, the rectangle
  between the two cells, as in spreadsheets and Notion. It is still stored as the two text ends, so undo, Shift+arrows
  and the caret work unchanged. Deleting or typing over it empties the cells (the grid keeps its shape); copying gives a
  paragraph per cell until tables go on the clipboard (E16-S04); formatting applies to all their text.
- **Tables are acted on whole:** a selection reaching from outside into a table deletes the table whole and joins
  nothing; the block menu, dragging and Tab over a selection act on the table, not a cell; a drop between cells lands
  after the table. The slash menu and Markdown shortcuts don't open in cells (a cell can't become a heading); a
  picture asked for in a cell goes below the table; blocks pasted into a cell become lines of text in it.

## Alternatives considered

- **Tab selects the next cell's text** (spreadsheet style): typing would replace it, surprising in a notes app. Notion
  puts the caret in the cell; we follow it.
- **A cell selection as a separate stored mode** (like block selection): would need its own undo bookkeeping; keeping it
  as the two text ends reuses everything.
- **Trim the partly selected cells when a selection reaches into a table**, as with text: leaves a table with some
  cells cut, rarely what was meant; Notion also takes the table whole.

## Consequences

- Every editor feature that acts on blocks now maps a row or cell to its table first (`table::whole_tables`).
- Until E16-S03 the app draws cells as indented text rows under a placeholder table row.
- Plain English: in a table, Tab jumps to the next cell (and makes a new row at the end), Enter starts a new line in
  the cell, arrow keys move around the grid, and dragging across cells selects whole cells. Things that work on
  blocks, like the block menu or dragging, treat the table as one block.
