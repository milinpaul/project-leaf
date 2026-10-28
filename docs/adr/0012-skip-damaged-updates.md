# 0012. Skip damaged saved updates instead of failing the load

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E03-S06

## Context

Each page is saved as a snapshot plus many small updates (ADR 0003, 0010). If one update is ever damaged (a bad
disk sector, a bug, a file edited by hand), loading the page would fail outright. The user would lose a whole
page over a few keystrokes. The product promises nothing is lost on a crash.

## Decision

- **Normal load first:** loading tries the snapshot plus all updates as usual.
- **Then recover:** only if that fails does `recover` import the snapshot and then each update on its own,
  skipping those Loro rejects.
  - `Store::load` logs a warning for each skipped update.
  - `Store::recover` also returns the skipped `seq`s, so the app can tell the user.
- **Any update, not only the last:** a damaged update may be skipped wherever it sits. After a skip, new saves
  are appended *after* it, so it doesn't stay last.
- **Nothing deleted:** skipped rows stay on disk; Leaf never deletes data it can't read.
- **Snapshots aren't skipped:** a damaged snapshot is still an error, because there is nothing to fall back on.

## Alternatives considered

- **Fail the load:** safest for the data, worst for the user, who can't open the page at all.
- **Skip only the last update:** matches a torn final write, but breaks as soon as anything is saved after it.
- **Delete damaged rows:** tidier, but it destroys evidence and anything a future version might still read.

## Consequences

- One damaged update costs only the edits it held. Later edits that built on those exact edits may not show.
- Compaction (E03-S05) must decide what to do with skipped rows; it must not quietly fold them away.
- Crash safety is proven by a property test: random edits, the autosave timer, and a crash at a random moment
  (the files copied while still open). Nothing older than the autosave delay is ever lost.
- Plain English: if a tiny piece of a saved page gets damaged, Leaf still opens the page, leaves out only that
  piece, and notes it in the log.
