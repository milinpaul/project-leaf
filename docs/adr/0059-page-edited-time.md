# 0059. Page edited time: a field in the workspace doc, stamped by a command undo skips

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E18-S01

## Context

The page top bar (E18) shows "Edited 3 minutes ago". Leaf didn't record that. The store's `updated_at` is when a
document was last saved on this computer: compaction changes it, and it won't sync. Page content lives in each
page's own document, but the sidebar, switcher and top bar read page facts from the workspace doc without opening
pages. The model never reads the clock (it must build for WebAssembly), and undo must stay one step per action.

## Decision

- **Where:** a new `edited_at` key (Unix ms) in each page's metadata in the workspace doc, next to `created_at`.
  Pages written before it existed read as their creation time; new pages start with `edited_at = created_at`.
- **What counts as an edit:** any change to the page's blocks (typing, formatting, moving blocks, and undo or redo,
  since undoing is editing too), and changing its title, icon or cover. Opening, scrolling, loading, adding the
  empty paragraph to a blank page, moving the page in the tree, trashing and restoring, and (later) starring don't.
- **How it's written:** a `WorkspaceCommand::MarkEdited { page, at }`, the caller supplying the time. It is its own
  commit with the origin `leaf:edited`, which the workspace undo manager ignores, so undoing a rename brings back
  the old title but never an older time.
- **Who stamps:** `WorkspaceEditor::stamp_edits(now)`. Each `PageDoc` remembers the version last stamped (the same
  bookkeeping as "last saved"), and the workspace editor remembers pages whose title, icon or cover it changed. The
  window calls `stamp_edits` on its 100 ms autosave tick, just before saving, so the time is within a tick of the
  change and goes to disk with it. One place catches every page command, present and future.
- **Two peers:** Loro keeps one value per key. A stamp written after seeing the other device's stamp always wins,
  which is the normal case: "the later wins". Two stamps made at the same moment, before either device saw the
  other, converge on one of them (both devices agree), and the next edit replaces it. Accepted until sync (E10).
- **Words:** `leaf-editor`'s `edited_ago(now, at)` gives "Edited just now" (under a minute), "Edited 1 minute ago",
  "Edited 2 hours ago", "Edited 100 days ago" (days never roll into weeks or months, as agreed with the owner).
  `edited_on(at, offset_minutes)` gives the hover label, "30 Oct 2026, 14:05"; the caller passes the local
  time-zone offset, so the crate stays clock- and zone-free.

## Alternatives considered

- **Use the store's `updated_at`:** wrong after compaction, local to one computer, and not part of the document.
- **Keep the time in each page's own doc:** the sidebar and top bar would have to open every page to show it.
- **Add `at` to every page command and write the time in the same commit:** dozens of call sites, and undoing
  that commit would restore the older time.
- **Stamp on every keystroke:** a workspace commit per key for no visible gain; the tick is precise enough.
- **One time per device, read as the latest:** exact under concurrency, but Loro peer IDs change each session, so
  the map would grow without bound. Not worth it before sync.

## Consequences

- Every way of changing a page is covered without each command remembering to stamp.
- While typing, the workspace doc gets at most one small commit per 100 ms tick; autosave and compaction (ADR 0048)
  absorb it.
- A time from a device whose clock is wrong is shown as written; the model doesn't second-guess the caller.
- Plain English: Leaf now remembers when you last changed each page, saves that with the workspace, and can say
  "Edited 3 minutes ago". Undo counts as a change, and the time is never rolled back.
