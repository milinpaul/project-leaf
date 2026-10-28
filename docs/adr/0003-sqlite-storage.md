# 0003. SQLite file for workspace storage

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E00-S03

## Context

Every edit must be on disk within a second, and a crash or force-quit must lose nothing older than that.
A workspace holds many page documents, plus later search and index data.

## Decision

Each workspace is a directory with one `workspace.sqlite` file (via `rusqlite`, SQLite bundled). It stores each
document as a Loro snapshot plus a log of small incremental updates, appended at most every 300 ms in WAL
mode and compacted into a fresh snapshot after 200 updates. Images sit next to it, named by content hash.
Schema in [BLOCK_MODEL.md](../architecture/BLOCK_MODEL.md).

## Alternatives considered

- **One file per page:** simple, but many small files make atomic multi-page changes and crash safety harder.
- **Loro snapshots only, rewritten on save:** slow for large pages and risky if a write is interrupted.
- **An embedded key-value store (sled, redb):** fine, but SQLite is more battle-tested, inspectable with
  standard tools, and gives us tables for indexes later.

## Consequences

- Appending small updates keeps typing cheap and crash-safe; transactions keep compaction atomic.
- One file is easy to back up and move; it is not human-readable, so markdown export covers that need.
- Forward-only, tested schema migrations are required from the first release.
- Plain English: your notes are saved continuously into a single robust database file, so a crash loses at most
  the last second of typing.
