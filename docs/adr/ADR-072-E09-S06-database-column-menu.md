# ADR-072. Database columns: header menus, and each database command its own undo step

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S06

## Context

A database's columns need to be added, renamed, retyped, moved, hidden, shown again, resized and deleted from the
table (E09-S06). Every column action must be one undo step, and changing a type must keep the stored values (ADR-066
answer 3). The model already has a `DatabaseCommand` for each action (E09-S02); what was missing was the menus, the
rules behind them, and undo that matches what the person did.

The database's undo history, like a page's, merged commands made within 500 ms into one step. That suits typing on a
page, where each key is a command. In a database each command is something done on purpose (a commit, a pick, a
column change), so two quick clicks were undone together.

## Decision

- **Menus.** Clicking a header opens that column's menu: its name (edited in place, kept on Enter, Escape or a click
  away), its type, move left and right, hide, delete. A "+" column at the end of the table opens a menu that adds a
  column of a type (named after it, "Text", "Text 2", …, then opened to be named) or shows a hidden one. Headers drag
  to reorder; header edges drag to resize. Both use gpui-component's data table, which reports the drop or the new
  widths once, when let go.
- **Rules in `leaf-editor`** (`database_view/columns.rs`): `column_menu`, `hidden_columns`, `add_column`,
  `rename_column` (blank keeps the old name), `set_column_type`, `step_column` and `move_shown_column` (moving works
  among the shown columns; hidden ones keep their place), `set_column_hidden`, `set_column_width`, `delete_column`.
- **Undo.** `DatabaseEditor` turns off time merging: each database command is its own undo step. An action made of
  several commands groups them with the new `Database::group` (Loro's undo groups): creating an option and picking it
  in the cell undo together. Cmd/Ctrl+Z and Shift+Cmd/Ctrl+Z on a database page undo and redo the database.

## Alternatives considered

- **Keep merging and accept quick clicks undoing together:** fails "every column action is one undo step".
- **Batch commands in the model (`apply_all`):** creating an option needs the new option's ID for the second command,
  which a fixed list can't express; a group around a closure can.

## Consequences

- The "Name" (title) column doesn't move, resize or open a menu; its name is the page title, renamed through each
  row.
- Undo on a database page covers the database only. Renaming a row's title is a workspace change and undoes with the
  workspace, as before.
- Option rename, colour, order and delete are still E09-S11.
- Plain English: click a column's name to rename it, change what kind of values it holds, move it, hide it or delete
  it; use "+" to add a column or bring back a hidden one. Undo takes back exactly one change at a time.
