# 0075. Database views: saved views as tabs, and a board layout

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E22-S01

## Context

The owner wants a Notion-style **board**: a database's rows shown as cards in columns, one column per value of a
property, with a card dragged to another column to change that value. And **saved views**: tabs above a database,
each a table or a board with its own settings, switched with a click. The spec lists board views under Phase 2.

Today a database has exactly one table view (ADR 0066). Its sort and filters are plain values in the `"view"` map, and
its hidden columns and widths sit on each column's schema entry. Databases saved by this version, and devices still
running it, must keep working (architecture rule 5: never drop unknown data).

The spike (`crates/leaf-model/tests/views_spike/`, Loro directly, no production code) tried the model below.

## Decision

Two new containers in the database's own document; nothing existing moves:

```text
"view":          LoroMap          the first view, ID "default": sort, filters, and new keys name, layout, group_by
"views":         LoroTree (flat)  tab order; node meta "id" = view ID. Nodes are never deleted.
"view_settings": LoroMap          view ID -> { name, layout, sort, filters, group_by, hidden, widths, hidden_groups,
                                  card_properties, ...unknown keys kept }
```

- **The first view stays where it is.** Its sort and filters stay in `"view"`, and its hidden columns and widths stay
  on the schema. A database saved today opens as one tab, "Table", and reading it writes nothing. A device on the
  older version keeps editing that table, and its edits merge with a newer device's other views.
- **Every other view keeps all its settings in its own map**, so hiding a column on a board doesn't hide it in the
  table.
- **Existence is kept apart from order**, as for columns (ADR 0066 correction): a view exists while its
  `view_settings` entry does; the tab tree only orders. So a tab deleted on one device and moved on another stays
  deleted.
- **The first view's tab node is made lazily**, the first time tabs are added to or reordered. Two devices can each
  make one; reading takes the first and the result is the same on both.
- **A board** groups by a select column. Groups:
  - "No value" first, then one group per option, in the column's option order;
  - cards in the database's row order, or the view's sort if it has one;
  - a cell naming an option that no longer exists counts as "No value", and its value is kept unread.
- **A card move** writes the cell and moves the row next to the card it was dropped by, as one undo step
  (`Database::group`, ADR 0072).
- **A layout from a newer version** (say `"calendar"`) keeps its settings untouched through this version's edits.
  This version shows it as a table.

## Findings

- All 14 spike tests pass. The two-device cases that converge to the result a person would expect:
  - views made on two devices are both kept;
  - a view deleted on one device and edited on the other stays deleted;
  - a tab moved on one device and deleted on the other stays deleted;
  - tabs reordered on two devices settle on one order;
  - the same card dragged to two groups ends in one group, the same on both;
  - a card dragged into an option deleted on the other device shows under "No value";
  - an older device's table edits merge with a newer device's board.
- The first view's tab node, made on two devices before they sync, reads once. In both peer orders the spike tried, a
  tab move made alongside it held. It isn't guaranteed in general: the worst case is a lost tab move, never a lost
  view or setting.
- A deleted grouping column leaves the board with nothing to group by. It asks for a property, and keeps `group_by`
  so undo brings the board back.
- Measured, release build, 10,000 rows, a five-option select, one in six rows empty:

| Step | Time |
|---|---|
| Group every row, reading the document | 38 ms |
| Group again from the editor's cached cells | 0.08 ms |
| First screen (25 cards) of every group | well under 1 µs |
| Move one card (value and place) | 0.37 ms |

  Switching between views of an open database is a pass over values the editor already holds, so it is instant.
  Opening a board reads like opening a table (ADR 0066): the first screen first, the rest in the background.

## Alternatives considered

- **Move the first view into `view_settings` on load:** a single place for every view, but a device on the older
  version would no longer see its sort, filters or hidden columns change. And two devices migrating at once would
  each make a copy.
- **Views as a movable list of maps:** one container for order and settings, but a delete against a concurrent move
  brings the element back (the ADR 0066 finding).
- **Each view its own document:** views are small and change with the database's columns. Keeping them in the same
  document means a view can't name a column the document doesn't know.
- **Card order kept per view, per group:** Notion keeps a manual order per board. Using the database's row order
  (when the view has no sort) needs no new data. A card dragged within a group moves its row, so a table view with no
  sort sees the same order.

## Owner's answers (2026-10-31)

All four as recommended:

1. **Phase:** Phase 1, after the rest of E09 (sort and filter, open a row, inline database).
2. **Group by:** select columns only, for now. Checkbox and multi-select can come later.
3. **Board column order:** follows the option order. Dragging a board column reorders the options (E09-S11).
4. **Per view:** everything, as in Notion. That covers layout, sort, filters, hidden properties, column widths,
   grouping, hidden groups and what cards show.

The defaults below stand as proposed.

### The questions as asked

1. **Phase.** The spec puts board views in Phase 2. Recommended: build E22 in Phase 1, right after E09 (sort and
   filter, open a row, inline database), since you asked for it now.
2. **What can group a board.** Recommended: select columns only at first. Checkbox (two columns) is easy to add later.
   Multi-select puts a card in several columns at once and makes dragging ambiguous, so leave it for later.
3. **Board column order.** Recommended: follow the option order. Dragging a board column reorders the options, so
   every view and picker agrees (needs E09-S11's option reorder). The alternative is an order kept per view.
4. **What each view keeps for itself.** Recommended: everything, as in Notion: layout, sort, filters, hidden
   properties, column widths, grouping, hidden groups, and what cards show. The alternative is to share hidden
   properties and widths across all views.

Defaults proposed without asking (say if you want otherwise):
- a new board's cards show the title only, with properties added from the board's menu;
- the first view can't be deleted (it is what a device on the older version shows), only renamed or changed;
- the open tab is remembered per device in `view.json`, like the last open page (E06-S19).

## Consequences

- `leaf-model` grows view commands; `leaf-editor` a board view model beside the table's; `leaf-ui` tabs and a board.
  Stories E22-S02 to E22-S05.
- Sort and filter (E09-S07) should write to the open view, so it lands on the first view's `"view"` map today and
  on any view once E22-S02 is in.
- A device on the older version shows only the first view; others it keeps, unread, until it updates.
- Plain English: a database gets tabs. Each tab is a way of looking at the same rows: a table, or a board of cards
  in columns by status (or any choice property). Drag a card to another column to change its status. Each tab
  remembers its own sorting, filters and which properties it shows.
