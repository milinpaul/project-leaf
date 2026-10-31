# ADR-XXX. Editing a column's options

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S11

## Context

E09-S05 lets a select or multi-select cell pick options and create new ones. The model could already rename, recolour
and delete an option (ADR-066), but not reorder one, and nothing in the app offered any of these. Deleting an option
in the model leaves its ID in the cells that picked it, where it reads as nothing. The story asks that deleting an option
clears it from every cell, as one undo step.

## Decision

- **Model.** A new `DatabaseCommand::MoveOption { column, option, index }` moves an option in the column's movable
  list. `index` counts options as they read (each once), so a list doubled by a merge still moves sensibly. The option
  commands move from `database/apply.rs` to `database/options.rs`.
- **Editor** (`DatabaseEditor`, `database_view/options.rs`):
  - `option_menu` gives an option's name, its colour and the nine palette colours;
  - `rename_option` trims the name; a blank name, or one another option of the column has (any case), keeps the old
    name, so two options never read the same;
  - `set_option_color` takes palette colours only;
  - `move_option` moves it;
  - `delete_option` takes the option out of every live row's cell (a multi-select cell is emptied in place, keeping its
    map, E09-S12), then deletes it, all in one `Database::group`, so undo brings back the option and every cell.
- **UI.** Each option in the picker (a table cell's and a row page's) has a "⋯" on hover. It opens the option's menu
  in place of the list: a name field (Enter or Escape goes back to the list, keeping the name), the colours, and
  "Delete". With nothing typed in the search field, an option drags onto another to take its place.

## Alternatives considered

- **Leave dead option IDs in cells** (the model's `DeleteOption` alone). Cheaper, and undo would still work, but
  exports, filters and other devices would keep carrying IDs that mean nothing; the story asked for the cells cleared.
- **Ask before deleting.** Undo brings everything back in one step, so a confirmation would only slow the owner down.
- **A separate pop-up beside the picker for the menu.** Showing it in place of the list keeps one pop-up, one place
  to click away from, and works the same in the table and on a row page.

## Consequences

- Deleting an option touches every row that has it: on a very large database that is many cell writes in one step.
- Filters that name a deleted option keep its ID and match nothing; they are not rewritten.
- Plain English: in a tag picker, hover a tag and click "⋯" to rename it, give it another colour or delete it. Deleting
  takes it off every row too, and undo puts it all back. Drag tags in the picker to change their order.
