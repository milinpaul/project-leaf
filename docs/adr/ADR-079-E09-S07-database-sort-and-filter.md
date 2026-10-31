# ADR-079. Sorting and filtering a database: a bar above the table, and typed filters written once

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S07

## Context

A database's sort and filters are already stored (E09-S02, ADR-066) and applied when the table is built (E09-S04).
E09-S07 adds the controls: sorting by one or more columns, and filters whose conditions suit each type. They are saved
with the database, so they survive restarts and sync. They must stay fast on 10,000 rows. Typing a filter's value
should narrow the table as you type, without making each letter its own undo step.

## Decision

- **A bar above the table:**
  - a chip for each sort ("↓ Plants") and each filter ("Watered is ticked");
  - "Sort" and "Filter" buttons;
  - "4 of 7" while filters hide rows.

  Each chip and button opens a pop-up under it:
  - columns to add;
  - the sorts, each with a direction button worded for its type ("A → Z", "9 → 1", "Old → New") and a remove button;
  - one filter, with its conditions as pills, its value's editor and "Delete filter".
- **Conditions by type, in `leaf-editor`** (`database_view/query.rs`, `filter_ops` and `op_label`):

| Type | Conditions |
|---|---|
| Text, URL | contains, does not contain, is, is not, is empty, is not empty |
| Number | =, ≠, >, < and the empty checks |
| Select | is, is not and the empty checks |
| Multi-select | has, does not have and the empty checks |
| Date | is, is before, is after and the empty checks |
| Checkbox | is ticked / not ticked |

- **Values:**
  - a text field (text, URL, number);
  - the column's options (select, multi-select);
  - a calendar (date, stored at local midnight);
  - two pills (checkbox).

  A new filter starts with its type's first condition and no value, which narrows nothing yet. A checkbox filter
  starts as "is ticked". "Is empty" drops the value; switching between conditions that both take one keeps it.
- **Typed values are drafts.** `DatabaseEditor` keeps the filter being typed and the table shows it at once. The
  database gets it on Enter, Escape or closing the pop-up, so a typed word is one undo step. Undo and redo drop a
  draft.
- **Every other change is one `SetSort` or `SetFilters` command**, so one undo step, saved and synced like any other.
- **Benchmarks** (`leaf-bench`, release, 10,000 rows sorted by a number and filtered to one status in five):
  - building the table: 5.2 ms;
  - one character typed into a text filter on top, then the table: 2.6 ms.

  Both fit in a frame. E09-S10 writes the budgets into `PRODUCT_SPEC.md`.

## Alternatives considered

- **Write every keystroke:** simple, but typing a word becomes many undo steps and many saves.
- **Debounce writes:** fewer writes, but undo steps would depend on typing speed.
- **gpui-component's popover and dropdown components:** the column menus already use the deferred, anchored panels
  in `database_page`; sharing them keeps one look and one way of closing (Escape, a click outside).

## Consequences

- Sorting and filtering work on the database's properties. The title ("Name") column, which is the row page's
  title, isn't offered yet.
- When saved views arrive (E22-S02), these controls write to the open view rather than to the database's single
  `"view"` map.
- Plain English: above each database there's a Sort button and a Filter button. Pick a column, and choose how to
  sort or what to keep: "Status is Doing", "Price > 10", "Due is before 1 Nov". The table changes as you type, and
  "4 of 7" says how many rows are showing. Your choices are saved with the database.
