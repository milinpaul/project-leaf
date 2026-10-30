# 0060. Favourite pages: a map from page to position in the workspace doc

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E18-S02

## Context

The page top bar (E18) has a star that adds the page to a Favourites section at the top of the sidebar. Favourites
belong to the workspace, are saved with it and will sync, so they must live in the workspace doc and change only
through commands. Two devices may star the same page, or different pages, at the same moment.

## Decision

- A root Loro map `"favourites"` in the workspace doc: key = `PageId`, value = position (`i64`).
- `AddFavourite { page }` stores the highest position plus one, so a new favourite goes last. Already starred:
  nothing changes. A page in the trash (or inside a trashed page) can't be starred.
- `RemoveFavourite { page }` deletes the key. Not starred: nothing changes.
- `Workspace::favourites()` lists entries by position, then page ID, leaving out pages in the trash and entries
  whose page is gone. Trashing keeps the entry, so restoring brings the page back to its place in the list.
  `DeletePagePermanently` removes the entries of every deleted page in the same commit, so undo restores both.
- Entries this version doesn't understand (a key that isn't a page ID, a value of another type) are skipped
  when listing but kept in the document (rule 5).
- `Workspace::import_changes` merges another copy's changes, for convergence tests now and sync later.

## Alternatives considered

- **A list of page IDs:** two devices starring the same page would leave it in the list twice, and reading would
  have to hide duplicates forever. A map gives "once" for free.
- **A movable list:** allows drag-to-reorder later, but has the same duplicate problem. Reordering favourites isn't
  asked for; if it is, positions can be rewritten in the map.
- **A `favourite: bool` in each page's metadata:** no order, and the sidebar would scan every page to find them.
- **Removing the entry on trash:** then restoring would lose the star, which the epic says it shouldn't.

## Consequences

- Starring is one small map write; listing reads one map and sorts at most a handful of entries.
- Two devices starring different pages at once can get the same position; both then sort by page ID, so they
  agree, though not necessarily in the order the stars were clicked.
- A device that starred a page another device deleted keeps a dangling entry, hidden from the list. Harmless and
  tiny.
- Plain English: Leaf keeps a list of starred pages with the workspace. Starring a page twice, or on two devices,
  still shows it once; pages in the trash disappear from the list and come back when restored.
