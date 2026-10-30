# 0068. Action items: a `task` block kind with its fields as props

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E19-S01

## Context

The owner wants action items in pages: something to do with a due date, an optional time and a priority (High,
Medium, Low or none), ticked off when done, and drawn differently from the plain to-do (E19). Pages are Loro
documents whose blocks keep their kind as a `type` plus plain values beside it (ADR 0007), and two devices editing
at once must converge without losing either change. The editor never reads the clock.

## Decision

- **A new kind, `task`** (`BlockKind::Task { done, due, time, priority }`), not more props on `todo`. A to-do stays
  the quick checkbox with nothing else to set; a task is its own thing in the slash menu and "Turn into", and
  turning one into the other is a normal kind change that drops the fields the new kind doesn't have.
- **Fields are props in the block's meta:** `done` (bool), `due` (`YYYY-MM-DD`), `due_time` (`HH:MM`, 24 h) and
  `priority` (`high`, `medium`, `low`). An absent prop means "not set". A time needs a date: setting one without a
  date is refused, and clearing the date clears the time.
- **A due date is a calendar date, a time is a wall-clock time**, with no time zone. A task due on Friday stays due
  on Friday on a trip, as in a paper diary. So there is no date library: `leaf-model`'s `DueDate`, `DueTime` and
  `Priority` validate and format themselves (leap years included). Whether a task is overdue is worked out by the
  editor from "today" passed in by the app (E19-S02).
- **Changed with `SetProp`** (`TaskDone`, `TaskDue`, `TaskTime`, `TaskPriority`), one undo step each, like any kind's
  props. Two devices changing different fields keep both: Loro doesn't record a write of an unchanged value, so
  rewriting the kind only touches the changed key. A test guards this; a per-field write path was tried and dropped
  as unnecessary.
- **Bad or newer data:** a `task` whose stored field can't be read is an error (as for other kinds), so it is never
  silently overwritten; fields this version doesn't know (say, a future `repeat`) are kept.
- Tasks can hold children, so notes and sub-items nest under one like under a list item.

## Alternatives considered

- **Extra props on `todo`:** fewer kinds, but every to-do would grow date and priority controls, and the owner asked
  for something visibly different.
- **A timestamp (Unix milliseconds) for the due time:** exact, but it shifts the day when the time zone changes and
  makes "due Friday, no time" awkward.
- **`chrono` or `time` for dates:** more than a calendar date needs, and one more dependency in a wasm-ok crate.

## Consequences

- Until E19-S02 and E19-S03, a task is drawn as a to-do; until E19-S04, Markdown writes it as a to-do line. Leaf's
  own clipboard already carries every field.
- Plain English: Leaf now has a place to store action items, each with a done tick, a due date, an optional time
  and a priority. Nothing shows them differently yet; the next stories add the look, the keys and the date picker.
