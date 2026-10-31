# ADR-008. Forward-only schema migrations and SQLite settings

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E03-S01

## Context

The workspace file (ADR-003) will outlive many versions of Leaf, so its schema must change safely over time. A
user may also open a workspace with an older Leaf than the one that last wrote it. Autosave needs a SQLite setup
where a committed save survives an app crash and saving stays fast.

## Decision

- **Migrations:** a numbered list of SQL steps in `leaf-store/src/migrations.rs`. Step `n` turns version `n` into
  `n + 1`; an empty file is version 0. The version lives in `meta.schema_version`. Steps are never edited once
  released; every change is a new step. Each step runs in one immediate (write-locked) transaction together with
  its version bump, so a failure leaves the file as it was and two processes can't run the same step.
- **Newer files are refused:** if the file's version is above what the app knows, opening fails with
  `SchemaTooNew` and nothing is written.
- **Connection settings:** `journal_mode=WAL`, `synchronous=NORMAL`, `foreign_keys=ON`. If WAL can't be enabled,
  opening fails.

## Alternatives considered

- **A migration crate** (`refinery`, `rusqlite_migration`): does the same in a few dozen lines we'd still need
  to test; not worth a dependency.
- **SQLite's `user_version` pragma** for the version: works, but the schema already has a `meta` table and the
  spec names `meta.schema_version`; one place is clearer.
- **Down migrations:** never run by users, so untested code. Backups are the way back.
- **`synchronous=FULL`:** also survives power cuts, at the cost of a disk flush on every save. The spec's
  "lose at most about a second" is about crashes, which `NORMAL` with WAL already covers.

## Consequences

- Any older workspace file can be brought up to date; tests cover migrating an empty file, re-opening, a failing
  step and a too-new file.
- An older Leaf can't open a newer workspace; it shows an error instead of damaging it.
- In-memory databases aren't supported by `Store` (no WAL); tests use temporary files.
- Plain English: the workspace file records which version of its layout it uses. Newer Leaf versions upgrade it
  one safe step at a time; an older Leaf refuses to touch a file it doesn't understand.
