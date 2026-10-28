# 0014. Incremental page view from Loro change events

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E04-S01

## Context

The screen draws a page from a flat list of rows: each block's depth, style (with list numbers) and formatted
text. Rebuilding that list means reading every block's text out of Loro, which grows with the page and would blow
the 16 ms frame budget on long pages. That is too slow to do on every keystroke. Loro already reports every
change as events, whether it comes from a command, undo, redo or (later) another peer.

## Decision

- **The model records what changed:** `PageDoc` subscribes to its Loro document's events.
  - Each event's path names the block node it happened under, so the page records those blocks.
  - Tree events also set a "structure changed" flag.
  - `take_changes()` hands them over as `BlockId`s, so Loro types stay inside `leaf-model`.
- **The editor's `PageView` caches each block's content** (its kind and its text runs) and on `refresh`:
  - re-reads only the changed blocks;
  - re-walks the outline for order, depth and numbering only if the structure changed or a block changed kind,
    reusing cached text;
  - otherwise patches those rows in place.
- **List numbers are computed during the walk, never stored:** consecutive numbered siblings count up, and
  anything else restarts the count.
- **`PageView::build`** makes a full view without consuming the change record. It is the reference the tests
  compare against.

## Alternatives considered

- **Rebuild on every change:** simplest, but its cost grows with the page.
- **The editor tracks what each of its commands touched:** misses changes from undo, redo and other peers.
- **Diff the old and new full views:** still reads everything.

## Consequences

- Typing costs one block's read. A property test checks, after every random edit, undo and redo, that the kept
  view equals a fresh build.
- Sync (later) gets the same incremental redraw for free, since remote changes arrive as the same events.
- The page holds a Loro subscription, released when the page is dropped.
- Plain English: when you type, Leaf only re-reads the paragraph you're typing in, not the whole page, so long
  pages stay fast.
