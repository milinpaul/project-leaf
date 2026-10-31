# ADR-085. Phase 1 is the current phase

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E00-S14

## Context

CLAUDE.md said "Phase 0: page editor. Do not build databases, links, sync or export unless the task says so", and
"No network access anywhere in Phase 0". By 2026-10-31 most of Phase 1 had already been built, story by story, each
as an exception the owner asked for: the database table and its views (E09, E22), page links and backlinks (E10),
the page top bar (E18) and the web bookmark block (E15). Twenty-five Phase 1 stories were done.

Phase 0's acceptance checklist was met except two checks done by hand in the running app (E08-S09): a 3,000-word
document and killing the app mid-typing. Automated tests already cover both. Cold start had passed since E08-S08,
though the acceptance record still said it failed. The backlog board called the first phase with an open story
"current", so it still showed Phase 0.

## Decision

The owner made Phase 1, the MVP, the current phase (2026-10-31).

- CLAUDE.md and CONTRIBUTING.md name Phase 1 as current, and Phase 2 work (sync, other database views, the web viewer)
  still needs the task to ask for it.
- The network rule becomes: no network access except `leaf-fetch`, and only once the owner allows web bookmark
  previews (ADR-080). Core crates never go online.
- Phase 0's open stories are finished alongside, not first. These are the hand checks (E08-S09), fuzzing (E08-S11),
  load limits (E08-S14), stricter lints (E08-S15), release builds (E08-S04) and two P2 fixes.
- The board marks the current phase explicitly (`CURRENT_PHASE` in `scripts/backlog.py`) rather than as the first
  with unfinished stories. The Bugs epic (E17) moves to Phase 1, as its notes planned.
- The spec ticks the performance budgets: cold start is 318 to 455 ms on the owner's Mac.

## Alternatives considered

- **Stay in Phase 0 until E08-S09 is done:** those checks are by hand and kept being put off, while every Phase 1
  story needed an exception to the rules.
- **Move the open Phase 0 stories into Phase 1:** a story's ID carries its epic, so moving stories means renaming
  them, and their links and PR references would break.

## Consequences

- New work no longer needs an exception for databases, links, Markdown import and export or web bookmarks.
- The next phase is set by changing `CURRENT_PHASE` and this decision's successor, not by finishing every story first.
- Plain English: Leaf's page editor is done, and the project is now building the MVP. A few clean-up and safety tasks
  from the editor phase continue alongside.
