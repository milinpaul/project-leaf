# 0036. Trash view: a dialog over the sidebar, and deleting from disk after the page tree is saved

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S06

## Context

Pages moved to the trash stay in the workspace, hidden (ADR 0001's soft delete, E01-S05). The owner asked for a Trash
page like Notion's: restore, delete forever and empty the trash. Deleting for good has to remove the pages'
documents from `workspace.sqlite` and from the search index, and must never lose a live page's blocks if Leaf crashes
half-way.

## Decision

- **Logic in the editor.** `WorkspaceEditor::trash_items`, `restore`, `delete_forever` and `empty_trash` turn into
  `RestorePage` / `DeletePagePermanently`. Deleting returns every page that went; the editor forgets their blocks,
  expanded state and place in the recent list, and closes the open page if it was one of them. `TrashList` filters
  the items with the quick switcher's matcher.
- **Order on disk.** After a permanent delete the shell saves the workspace document first (it no longer lists
  the pages), then deletes the pages' documents (`Store::delete`) and drops them from autosave. A crash in between
  leaves unused documents in the file, never a page in the tree whose blocks are gone. Search drops the pages on its
  next pass, because it indexes only what the saved workspace lists.
- **UI.** A Trash row at the foot of the sidebar opens a dialog: filter field; each page with its icon, where it
  lived and how long ago it was trashed; Restore and Delete forever on hover; Empty trash at the foot. Both deletes
  ask first in an alert with a red button naming what goes. Clicking a page opens it with a banner offering Restore
  page or Delete forever.

## Alternatives considered

- **A full-page Trash view in the page area:** more room, but it competes with the open page and needs its own
  navigation; a dialog is how Notion does it and keeps the page where it was.
- **Deleting documents before saving the tree:** simpler, but a crash then leaves a page listed in the tree with no
  blocks.
- **Never deleting documents (just hiding them):** the file would grow forever, and "delete forever" would be a lie.

## Consequences

- The search index needs no new code path: deleted pages fall out like trashed ones.
- Unused documents left by a crash are harmless; a later compaction story can sweep documents no page refers to.
- Plain English: click Trash at the bottom of the sidebar to see pages you binned. Put one back, or delete it for
  good (Leaf asks first). "Empty trash" deletes them all. Opening a binned page shows a red strip saying so.
