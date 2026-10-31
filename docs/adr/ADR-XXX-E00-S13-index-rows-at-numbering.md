# ADR-XXX. Decision record index rows are added when records are numbered

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E00-S13

## Context

ADR-076 numbers new decision records after merge, but each story branch still added its record's row to the end of
the index in `docs/adr/README.md`. Any two PRs open at once with a record each clashed on that line, and every one
needed a manual merge.

## Decision

- Story branches leave the index alone.
- `scripts/adr_number.py`, which the mark-done PR already runs on the newest `main`, adds a row for each record it
  numbers, after the last row. The row is built from the record's heading, `Type` and `Status` lines.
- A row a branch did add the old way (`[ADR-XXX](…)`) is renamed in place, not added twice, so PRs already open keep
  working.

## Alternatives considered

- **A merge driver (`merge=union` in `.gitattributes`):** works for local merges, but GitHub's merge button and its
  conflict check ignore it.
- **Generating the whole index on every PR:** every PR would still change the same file.

## Consequences

- The index changes only in mark-done PRs, which already run one at a time on the newest `main`.
- A record is missing from the index until its story is marked done; the files in `docs/adr/` list it meanwhile.
- Plain English: the list of design decisions is now updated automatically when a decision gets its number, so
  open pull requests no longer collide on it.
