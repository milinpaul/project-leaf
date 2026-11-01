# ADR-095. A preview card when the pointer rests on a page link

- **Status:** Accepted (owner accepted the recommended behaviour, 2026-11-01)
- **Type:** Feature
- **Date:** 2026-11-01
- **Story:** E10-S12

## Context

The owner asked for Notion's hover preview: resting the pointer on a link to a page shows a small card with that
page's start, so you can tell it's the right page before opening it. Notion added it in October 2023, and many
people said it got in the way on pages full of links and asked for a way to turn it off. A link's target page is
often not loaded (Leaf loads a page's document when it opens), and website previews would need to go online,
which ADR-080 restricts.

## Decision

- **When:** after 0.6 s on a page link (or a link to a block); never while a mouse button is down. It goes 0.2 s after
  the pointer leaves both the link and the card (so the pointer can cross to it), and at once on any key, a click or a
  scroll. Esc closes it and does nothing else.
- **What:** the page's path, icon and title, then its first 6 non-empty lines as plain text drawn like their blocks,
  smaller; "…" when there's more. For a link to a block, from one line above that block, which is highlighted. A
  page in the trash: title and "In trash". A deleted page: no card. Web links: no card.
- **Where it comes from:** `WorkspaceEditor::page_preview` (leaf-editor, plain and tested). When the target isn't
  loaded, the shell loads it from disk when the pause ends: a single page read, well under the pause.
- **Where it goes:** under the link's line, or above it when there's no room below (the slash menu's rule).
- **Keys:** a keystroke interceptor closes the card, because keys bound to actions (arrows, Esc) never reach the page's
  own key listeners.
- **Off switch:** Settings › Appearance › "Preview pages on hover", kept in `settings.json` (on unless turned off).

## Alternatives considered

- **Read the preview from the search index** (no page load): the index keeps plain text without block kinds, so the
  card couldn't draw headings, lists or to-dos, and it lags a save by a moment.
- **Show only while holding Cmd**, as some asked of Notion: hides the feature from anyone who doesn't know; the off
  switch answers the same complaint.
- **Preview web links too:** needs network access, which only web bookmarks have, and only when allowed (ADR-080).

## Consequences

- Resting on a link loads its page into memory; pages loaded this way stay loaded like opened ones.
- The off-screen screenshot tool runs no timers, so its `hover=B` option ends the pause itself
  (`Shell::show_link_preview_now`).

In plain English: hold the pointer still on a link to a page and, after a moment, a small card shows the start of that
page. It disappears as soon as you move away, type or click, and you can turn it off in Settings.
