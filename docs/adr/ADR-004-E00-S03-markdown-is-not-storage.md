# ADR-004. Markdown is for import and export, not storage

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-28
- **Story:** E00-S03

## Context

Tools like Obsidian store notes as markdown files, which users like for ownership. Leaf has rich blocks (to-dos,
nesting, image sizes, later databases), needs conflict-free merging for sync, and must never drop data.

## Decision

The source of truth is the Loro documents in SQLite (ADR-001, ADR-003). Markdown is an exchange format: export a
page or whole workspace, and import markdown files (Phase 1). Pasting markdown into the editor converts it.

## Alternatives considered

- **Markdown files as storage:** human-readable, but loses block IDs, nesting details and merge history, and
  turns every save into a lossy round trip.
- **Markdown plus sidecar metadata files:** keeps more data, but two sources of truth drift apart.

## Consequences

- No lossy round trip on every save; sync and undo work on the real structure.
- Ownership comes from a documented format plus reliable export, not from the files themselves.
- Export quality matters and is snapshot-tested (`insta`).
- Plain English: your pages live in Leaf's own format so nothing is lost, and you can export them to markdown
  whenever you want.
