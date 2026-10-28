# 0009. One app per workspace via an OS file lock

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-28
- **Story:** E03-S02

## Context

If two Leaf windows or processes open the same workspace, each holds its own copy of the documents in memory and
autosaves over the other's edits. Leaf must refuse the second one. The guard must never get stuck: after a crash
or a force-quit the owner has to be able to reopen their workspace without deleting anything by hand.

## Decision

Each workspace folder has a `leaf.lock` file. Opening takes an exclusive operating-system lock on it
(`File::try_lock` from the Rust standard library: `flock` on macOS and Linux, `LockFileEx` on Windows) before the
database is touched, and holds it until the workspace is closed. If the lock is taken, opening fails with
`AlreadyOpen`. The OS releases the lock when the process exits for any reason. The file itself stays and means
nothing on its own.

## Alternatives considered

- **A lock file that holds a process ID**, checked on open: a crash leaves it behind, and deciding whether that
  process is still alive (or reused its ID) is unreliable across platforms.
- **SQLite's exclusive locking mode:** it only locks once the first write happens and blocks rather than failing
  clearly. It also wouldn't cover the `assets/` folder.
- **The `fs4` or `fd-lock` crates:** these do what the standard library now does, so they would only add a dependency.

## Consequences

- A crashed app never leaves the workspace locked.
- OS locks on network drives can be unreliable; workspaces on network shares are not a Phase 0 target.
- On Unix the lock is advisory: it only stops other Leaf instances, not unrelated programs editing the files.
- Plain English: while Leaf has a workspace open, another Leaf window that tries to open it gets a clear
  "already open" message. If the app crashes, the workspace is free again at once.
