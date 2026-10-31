# ADR-086. A database row opens as a page with its properties under the title

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S08

## Context

Each database row is a page (ADR-066). E09-S08 opens it from the table and shows the row's properties on it, editable,
above the page body, with the breadcrumb showing the database.

## Decision

- **Opening:** the table's title cell shows an "Open" button while the pointer is over it. A click opens the row's
  page. Double-click still edits the title in place.
- **Properties:**
  - listed under the page's title, each a name with its type's icon, then its value;
  - every column, in column order, hidden ones too, since the page is where a row is read in full;
  - an empty value reads "Empty".
- **Editing:** a click on a value edits it in place, with the table cell's rules (`CellEditor` and the editing
  calls in `leaf-editor` `database_view/edit.rs`, with the row's page as the row):
  - text, URL and number in a field: Enter or clicking away keeps it, Escape drops it, and a bad number keeps the
    field open;
  - a checkbox toggles;
  - select and multi-select open the same option picker;
  - a date opens the calendar.

  So a change on the page is the same command, the same undo step and the same value as in the table.
- **The breadcrumb** shows the database page, as row pages sit under it (E09-S03).

## Alternatives considered

- **Only the shown columns:** hidden columns would have no place to be read or edited at all.
- **A side peek instead of a full page** (Notion's default): needs a second editor beside the table. The story asks
  for the page; a peek can come later.

## Consequences

- A property change is one of the database's undo steps. On the row page, Cmd/Ctrl+Z undoes the page's text, as on any
  page. A property change is undone from the database's page. Sharing one undo across a page and its database
  would need a combined history.
- Plain English: hover a row's name in a table and click "Open" to see the row as a page. Its properties (status,
  dates, tags and so on) are listed at the top, and you can change them there; the table shows the change.
