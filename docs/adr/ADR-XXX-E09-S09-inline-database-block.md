# ADR-XXX. The inline database block follows its database

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E09-S09

## Context

ADR-066 decided that a database can sit inside a page as a `database` block, and (answer 4) that deleting the block
trashes the database and its rows, with undo or restore bringing them back. It left open how that is stored, where the
database page lives, what copying the block does, and how the trash state follows a block whose history is the page's,
not the workspace's. The owner answered two of these on 2026-10-31: the inline database appears in the sidebar under its
page (its rows stay out, as always), and copying the block or duplicating it gives **a link to the same database**,
never a second table or a copy of the data.

## Decision

- **The block.** `BlockKind::Database { page }`, stored as type `database` with a `page` prop (the database page's ID).
  No text, no children. It never turns into another kind and no block turns into one: it is only inserted. A block
  that lost its `page` in a merge is repaired to a paragraph.
- **The database page** is an ordinary database page filed under the page the block is on, so the sidebar lists it
  there. `/database` asks for one where it was typed; the workspace (`WorkspaceEditor::make_requested_database`) makes
  the page and the block goes in its place (an empty line gives way, with a new line below).
- **Trash follows the block.** The page's undo history holds the block; the database page's own changes are made
  outside the workspace's undo (`apply_unrecorded`). After each page change the shell calls
  `WorkspaceEditor::sync_inline_databases`: a database whose block left the page goes to the Trash (with its rows, which
  live under it), and one whose block came back (undo, redo) comes out of it, if it went to the Trash on its own. The
  page's editor notes which databases it showed when it was first drawn, so only changes after that count.
- **Copy and duplicate give a link.** Copying the block (clipboard, Markdown) and duplicating it (`DuplicateBlocks`)
  give a paragraph holding one page link to the database.
- **The UI.** The block draws the database's title with an "Open" button, its sort and filter bar, its table (ten rows
  tall, then it scrolls) and "New", all editing as on the full page. The shell keeps a view per database shown and
  makes the one pressed (or being drawn) the active one, which the table actions work on. The table's wrapper binds
  the page's keys to nothing, so they never reach the page while the table has the keyboard. A press beside the title
  selects the block, so Backspace or the block menu deletes it.

## Alternatives considered

- **Store the database inside the page's document.** One document per database is ADR-066's model; mixing them would
  give two formats for the same thing.
- **Mark inline database pages ("inline: true") and reconcile from that.** The block is already the mark; following the
  block's comings and goings needs no new field and leaves database pages that were made full-page alone.
- **Record the database page in the workspace's undo.** The workspace's undo could then delete a database whose block
  is still on the page. Following the block keeps one undo, the page's.
- **Copy pastes a second inline table of the same database.** The owner chose a link: a database is shown in one place.

## Consequences

- Deleting a block within the merge interval of making it undoes both together, as with any two quick edits.
- A database page trashed from the sidebar while its block stays shows a "This database is in the Trash." line in the
  page; one deleted for good shows "This database was deleted."
- Search and export of inline databases are E09-S10's.
- Plain English: typing `/database` puts a small table in the page, also listed under the page in the sidebar. It works
  like the full table. Delete the block and the table goes to the Trash; undo brings it back. Copying it gives a link
  to the same table.
