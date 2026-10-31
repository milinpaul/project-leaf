# ADR-011. Autosave throttles from the first change, with a clock-free policy

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E03-S04

## Context

The spec promises that every edit is on disk within 1 s, with no save button. Saving on every keystroke would
write far more than needed. The rule that decides when to save has to be testable without real waiting, and the
storage crate must not depend on the UI's timers.

## Decision

- **When:** each document is saved at most 300 ms after its **first** unsaved change. Later changes don't move
  the deadline (a throttle).
- **Blur and quit:** everything pending is saved at once.
- **No clock inside:** `leaf-store`'s `Autosave` reads no clock and starts no threads. The app tells it about
  changes with the time, sets a timer for `next_deadline()`, and saves what `due(now)` lists. Tests pass plain
  numbers as the time.
- **Failures:** a document stays pending until its write succeeds.

## Alternatives considered

- **Debounce** (save 300 ms after the *last* change): continuous typing would never save, which breaks the 1 s
  promise.
- **A background thread in `leaf-store`:** that means real-time tests, races with the UI thread over the
  documents, and a second SQLite connection.
- **Save on every command:** simplest, but it writes once per keystroke.

## Consequences

- At most about three small writes a second per document while typing, and none when idle.
- The app (E03-S08) must call `changed` after each command and run the timer. Forgetting to call `changed` would
  delay saving until blur or quit.
- Plain English: Leaf saves a page a moment after you start changing it, and again on the next change after that.
  It never waits for you to stop typing, and it saves everything when you switch away or quit.
