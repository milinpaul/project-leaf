# ADR-065. Page top bar: breadcrumb, edited time, star and page menu above every page

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-30
- **Story:** E18-S03

## Context

The owner asked for Notion's bar across the top of a page, without the parts that need sync or sharing: where
the page sits, when it was last edited, a star for Favourites and a `⋯` menu. E18-S01 records the edited time
(ADR-059) and E18-S02 the favourites (ADR-060); this decides how the bar shows them.

## Decision

- **Layout:** a 44 px row above the open page (`shell/top_bar.rs`), only while a page is open. Left: the breadcrumb.
  Right: "Edited 3 minutes ago", the star, `⋯`. While the sidebar is hidden, its ☰ button sits at the bar's left
  end (with no page open it still floats in the corner, E06-S18).
- **Breadcrumb** (`WorkspaceEditor::breadcrumb`, `Crumb`): the page's ancestors from the top, then the page, with
  icons and "Untitled". Paths longer than four keep the first page, a "…" gap and the last two. Each crumb opens
  its page. When room runs short the open page's title shortens first (it shrinks 50 times faster than its
  ancestors, which are also capped at 180 px), and the row is clipped so it never covers the controls.
- **Edited label:** `edited_ago(now, edited_at)`, redrawn every 30 seconds by a timer on the window. A page with
  edits not yet stamped (the save timer stamps them within 100 ms) already reads "Edited just now". Hovering shows
  `edited_on(at, offset)`, the exact local time.
- **Local time zone:** read with `chrono::Local` in `leaf-ui`. `chrono` was already in the build through
  gpui-component with its `clock` feature, so this adds no crate; `leaf-editor` keeps taking the offset as a
  number and stays free of clocks and time zones.
- **Star:** outline, or filled in amber while the page is a favourite; a click toggles `set_favourite`.
- **`⋯` menu** (amended in E18-S04): the page menu. Its entries are one list in `leaf-editor`
  (`WorkspaceEditor::page_menu`: action, label, icon, shortcut, red or not, enabled, separator) that the sidebar's
  right-click menu uses too (`shell/page_menu.rs` draws both and runs each action in one place): Add to / Remove
  from Favourites, Rename, Move to Trash, and in the top bar a footer with when the page was created and last
  edited; the sidebar adds "Add page inside". Rename from the top bar selects the page's title to type over; from
  the sidebar it edits the row. Move to Trash opens the nearest page still there (its parent, the page after, the
  one before, or the first page; `trash_and_open_next`) and shows a "Moved to Trash" notice whose Undo restores the
  page and reopens it. Workspace changes have no Cmd+Z yet, so the notice is the undo.
- **Full width** (amended in E18-S06): a top-bar-only entry with a switch (`PageMenuEntry::switch`), like Notion's.
  It is a setting of the page, `full_width` in its meta in the workspace doc (`SetPageFullWidth`; stored only while
  on, so older pages read as off), so it is saved and will sync. On, the title and blocks drop the 720 px reading
  cap and fill the page area within its side margins; the cover is unchanged. It changes layout, not words, so it
  doesn't move the edited time. Kept per page rather than as an app setting because some pages (tables, wide code)
  want room and others read better narrow.

## Alternatives considered

- **Bar inside the page's scroll area:** it would scroll away with the page; Notion keeps it fixed, and so do we.
- **Edited time from the store's `updated_at`:** local to this computer and moved by compaction (ADR-059).
- **Reading the time zone by hand (`localtime_r`):** needs `unsafe`, which the workspace forbids.
- **Ellipsis on any path over three:** hides too much; four steps fit comfortably at normal widths.

## Consequences

- The page starts 44 px lower than before; the page header test now measures from the bar.
- The edited label is at most 30 seconds behind while nothing else redraws the window.
- Plain English: every page now has a slim bar on top showing where it lives, when it was last changed, a star to
  keep it handy, and a `⋯` menu that will grow.
