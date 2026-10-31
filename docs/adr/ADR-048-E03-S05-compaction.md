# ADR-048. Compaction: the save past 200 updates folds them into a fresh snapshot

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E03-S05

## Context

Each save appends one small Loro update to `doc_updates` (ADR-010), so a page edited for months collects
thousands of rows, and every open replays them all. `BLOCK_MODEL.md` asks for a fresh snapshot once a document has
more than 200 updates, in one transaction. ADR-012 adds a constraint: an update that can't be read is skipped on
load but kept on disk, so compaction must not quietly fold it away.

## Decision

- `Store::save` calls `Store::compact` after appending an update. `compact` counts the document's updates (one
  indexed query) and does nothing at 200 or fewer.
- Past 200, inside one immediate transaction: read the snapshot and updates, fold them with
  `leaf_model::fold_saved`, write the new snapshot, then delete the updates, then commit. Any failure rolls back to
  the old snapshot and every update.
- `fold_saved` works on bytes in a plain `LoroDoc` (import, then export a full snapshot), so it serves pages and the
  workspace alike, keeps the whole history (copies elsewhere can still merge), and carries data from a newer Leaf
  over untouched.
- It reads from disk, not from the open document: what is folded is exactly what was saved.
- If an update is damaged, or waits for a change that isn't saved, nothing is folded; a warning is logged and the
  rows stay, as ADR-012 requires.
- A compaction error doesn't fail the save, which already succeeded. It is logged, and the next save tries again.
- The document's `updated_at` is set to its last edit time, so "last saved" and "most recently edited" are
  unchanged. Its saved version's `seq` drops to 0, so search re-reads the page once, which costs a little and
  is harmless.

## Alternatives considered

- **Snapshot the open document:** no reread, but a document loaded with skipped damaged updates would overwrite
  them, and it would tie compaction to whichever type is open.
- **Compact on open or on quit:** opening would be slower for the page about to be shown, and a crash before quit
  would skip it. Doing it on the save that crosses the limit spreads the cost.
- **Shallow snapshot (drop history):** smaller, but other copies couldn't merge with it any more, which future sync
  needs.

## Consequences

- Loading a page replays at most 200 small updates after its snapshot.
- Once in every ~200 saves, a save also reads and rewrites one document. That costs milliseconds, off the typing
  path.
- A document with a damaged update keeps trying to compact on every save until the damage is dealt with. This is
  rare, and it only costs that document's load time.
- Plain English: Leaf saves each change as a small note. When a page has more than 200 notes, Leaf rewrites the
  page as one clean copy and throws the notes away, all in one step. If anything goes wrong part way, nothing
  changes. Notes Leaf can't read are never thrown away.
