# 0069. Database page: gpui-component's data table, editing rules in leaf-editor

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S05

## Context

A database page (ADR 0066) shows its rows as a table: a header with each column's name and type icon, one row per
row page, a "New" line under the last row, and sideways scrolling when the columns are wider than the page. Cells edit
in place, each by its type. It must stay fast for 10,000 rows (owner's answer 5 on ADR 0066), and the architecture rules
keep editing logic out of `leaf-ui` (rule 3).

## Decision

- **The table is gpui-component's `DataTable`**, filled through a `TableDelegate` (`leaf_ui::shell::database_page::table`)
  that draws the header and each cell from `leaf_editor::TableView`. The table draws only the rows on screen, scrolls
  both ways, selects cells, and moves the selection with the arrows, Tab and Home/End.
- **What each cell does is decided in `leaf-editor`** (`database_view/edit.rs`): `cell_editor` says which editor a cell
  opens (a text line for text, URL and number; a toggle for a checkbox; the option picker for select and multi-select;
  a calendar for a date; nothing for a type from a newer version). `commit_text`, `toggle_checkbox`, `option_picker`,
  `toggle_option`, `create_option` and `set_date` turn what the person did into `DatabaseCommand`s. The UI only draws
  and forwards.
- **Editing:** double-click a cell, or select it and press Enter. Enter (or clicking away) commits text and Escape drops
  it. A number that doesn't read as one keeps its editor open. A select's picker closes once an option is picked; a
  multi-select's stays open for the next. Typing a name that isn't an option offers "Create", in the next palette
  colour. A picked date is stored at local midnight.
- **Reading:** the first screen of rows is read when the page opens; the rest are read 500 rows at a time on a 16 ms
  timer until all are in.
- **Looks:** the table is as tall as its rows, so "New" sits under the last one, and a long table scrolls inside the
  page. Cells have no padding from the table, so each draws its own and a subtle divider on its right in the row-line
  colour. Option colours come from `leaf_ui::theme::option_colors`, soft in both light and dark.
- A new database starts with one multi-select column, "Tags", so there is something to fill in.

## Alternatives considered

- **Our own table on GPUI's `uniform_list`:** full control, but we would rebuild cell selection, keyboard movement,
  sideways scrolling and the sticky header that the data table already has.
- **Editing logic in the delegate:** quicker to write, but untestable without a window and against rule 3.

## Consequences

- The table's own key bindings (arrows, Tab, Escape to clear the selection) come with it. Escape for an open cell
  editor is bound in a context on the cell itself, inside the table's, so it wins over the table's Escape.
- The table reserves room for a sort icon in each header; the header box takes the column's exact width to reach over
  it so the dividers line up. A future gpui-component update could move this.
- Renaming, recolouring, reordering and deleting options is E09-S11; adding, renaming or retyping columns from the
  header is E09-S06.
- Plain English: a database page now looks and works like a spreadsheet-style table. You can type in cells, tick
  boxes, pick or create coloured tags and choose dates, in light and dark.
