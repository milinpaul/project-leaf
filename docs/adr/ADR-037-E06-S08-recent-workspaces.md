# ADR-037. Recent workspaces live in the app settings; a missing one is flagged, not dropped

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S08

## Context

`PRODUCT_SPEC.md` asks to "create or open a workspace (a local directory)". The welcome screen (E06-S09) needs the
workspaces opened before, and a plain launch should reopen the last one instead of always the default folder.

## Decision

- The list lives in the app settings file (ADR-029) as `recent_workspaces`: path and last-opened time, most recent
  first, at most 10, one entry per folder (paths are compared after resolving `..` and links).
- A workspace is remembered as soon as it opens (and the settings saved then), so a crash still reopens it.
- At launch: a path on the command line wins; otherwise the last workspace if its folder is still there; otherwise
  the default `~/Documents/Leaf/My Workspace.leaf`.
- A workspace whose folder is gone stays in the list flagged as missing (it may be on a drive that isn't plugged
  in); only "Remove from list" forgets it, and never touches the folder.
- Entries this version can't read are skipped, not treated as a damaged file.

## Alternatives considered

- **A separate `recent.json`:** one more file to keep in step; the settings file already has the right rules
  (unknown fields kept, damaged file set aside).
- **Dropping missing workspaces automatically:** a drive that's simply unplugged would lose its entry.

## Consequences

- The welcome screen (E06-S09) can list workspaces with no further storage work.
- Folders whose names aren't valid Unicode aren't remembered (the file stays plain text); they still open.
- Plain English: Leaf now reopens the workspace you used last, and remembers your last ten workspaces for the
  welcome screen. A workspace that's been moved shows as missing until you remove it from the list.
