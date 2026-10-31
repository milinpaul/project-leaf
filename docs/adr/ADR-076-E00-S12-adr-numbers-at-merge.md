# ADR-076. Decision records are numbered when they merge, by a workflow

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E00-S12

## Context

Decision records were named `NNNN-title.md`, each branch taking "the next free number". Agents work on several
branches at once, so two branches often took the same number (0061, 0063 and 0074 clashed), and every clash cost a
rename and a merge conflict. The owner wants the records listed in the order taken, with the story in the name, and
never two with the same number (2026-10-31).

## Decision

- Every record is named `ADR-NNN-<story>-<title>.md`: a three-digit number in the order taken, then the story (or its
  epic, or nothing), then a short title. All existing records were renamed this way, and every mention follows
  (`ADR-068`).
- On a branch, a new record has no number: `ADR-XXX-<story>-<title>.md`, headed `# ADR-XXX. Title`, listed in the index
  as `[ADR-XXX](...)` and mentioned as `ADR-XXX-<story>`. One new record per story keeps the mention unique.
- When it reaches `main`, the `ADR numbers` workflow (`.github/workflows/adr-numbers.yml`) runs
  `scripts/adr_number.py`: each waiting record gets the next number after the highest one, every link and mention in
  the repository's text files is updated, and the workflow pushes that one commit to `main`. Runs are queued one at a
  time and never cancelled, and a push that loses a race starts again from the newest `main`.

## Alternatives considered

- **Each branch picks the next number:** the clashes this replaces.
- **A CI check that fails on a duplicate number:** reports the clash but doesn't prevent it; the second PR still has
  to rename. Tried and dropped.
- **Date and time in the name** (`ADR-2026-10-31-1405-…`): never clashes and needs no automation, but the owner
  preferred short numbers.

## Consequences

- The numbering job is the one CI job allowed to write: `contents: write`, for its own commit to `main` (an exception
  to ADR-073's read-only CI, scoped to that job). If `main` is ever protected against direct pushes, the workflow's bot
  must be allowed to push, or the numbering waits for a manual run.
- That commit is made with the workflow's own token, so it doesn't start another CI run; it only renames and edits
  text.
- For a moment after a merge, `main` holds `ADR-XXX` names; links still resolve, because the file and its mentions
  carry the same name until the workflow renames them together.
- Plain English: new decision records get their number automatically when their PR is merged, in the order they land,
  so two can never share a number and nobody has to fix clashes by hand.
