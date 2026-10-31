# 0074. Workspace files stay inside the folder: no links, no device names

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S13

## Context

`SECURITY.md` names "anything that lets Leaf read or write outside the workspace folder you chose" as in scope.
A review of every place Leaf builds a path from stored or typed data found:

- Files inside a workspace have fixed names (`workspace.sqlite` and its `-wal`/`-shm`, `leaf.lock`, `view.json`
  and its `.tmp`, `search/index.sqlite`), or are named by an `AssetId`: a typed 32-byte hash, shown as 64 hex
  characters and never read back from a stored string, so it can't hold `..` or a slash.
- But any of those names can be a **symlink** in a workspace someone else made (a shared folder, an unzipped
  archive). Leaf follows it on write, so the workspace could make Leaf create or overwrite any file the owner can
  write.
- New workspace names already refused path characters and dot-names, but accepted **Windows device names**
  (`CON`, `NUL`, `COM1`, also with an ending such as `nul.leaf`), which Windows can't create or open as folders.

## Decision

- `Workspace::open` (and `create`) refuses a folder whose top level, `assets/` or `search/` holds a link
  (symlink, or on Windows a junction), with `StoreError::LinkInWorkspace(path)`. The check runs before the lock
  file is opened, because that open would already follow a link. Nothing in the folder is changed, and the welcome
  screen explains it in plain words. The owner chose refusing over repairing (2026-10-31).
- The workspace folder itself may be a link: the owner picked it on purpose.
- `new_workspace_path` refuses Windows device names in any case, with or without an ending, with
  `NameProblem::Reserved`, on every OS, so a workspace made on a Mac still opens on Windows.

## Alternatives considered

- **Open, but never write through links** (replace a linked file with a real one on write, treat linked folders
  as read-only): keeps such workspaces usable, but every write site needs its own guard and its own tests, and a
  new write site can silently miss it.
- **Allow links:** would support keeping `assets/` on another drive, which no one has asked for, at the cost of the
  whole class of problems above.
- **Check deeper folders too:** Leaf never writes below `assets/` and `search/`, so links deeper down can't be
  followed by a Leaf write.

## Consequences

- A workspace with a link in it no longer opens until the link is removed. That's rare in practice and the
  message says what to do.
- A new file Leaf writes must go in the top level, `assets/` or `search/`, or the check must be extended.
- The check costs one directory listing of three folders on open, the largest being `assets/`.
- The symlink tests are Unix-only: creating links on Windows needs extra rights. Rust's standard library treats
  Windows symlinks and junctions alike as links, so the same check should apply there, but CI doesn't prove it.
- In plain English: a workspace can't trick Leaf into writing anywhere outside its own folder, and Leaf won't let
  you name a workspace something Windows can't handle.
