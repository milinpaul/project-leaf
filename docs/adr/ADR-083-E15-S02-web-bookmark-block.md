# ADR-083. The web bookmark block: an address, a preview off the undo history, and an empty text container

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E15-S02

## Context

ADR-080 settled how Leaf fetches web bookmark previews. This story adds the block itself to the page model: what it
stores, how the fetched preview reaches it, and how it turns into text and back. The block has no text of its own,
but turning a bookmark into a paragraph must keep its address as linked text, and a block that is only one link may
become a bookmark.

The page undo property test found a problem with the first version: insert a bookmark, turn it into a paragraph,
undo both, redo both, and the paragraph comes back empty. It is the Loro behaviour ADR-007 describes: a container
created in a later step than its block loses what is written into it when undo re-creates the block and redo
replays the step. The conversion created the paragraph's text container and wrote the address into it in one step.

## Decision

- **Kind:** `BlockKind::WebBookmark { url, preview: WebPreview }`, stored as type `web_bookmark` with plain props in
  the block's meta (ADR-007): `url`, then `title`, `description`, `image` and `favicon` (asset IDs) and `fetched_at`
  (Unix ms) once fetched. No text, nothing nested.
- **Addresses:** `check_web_address` accepts `http`/`https` with a host, no spaces or control characters, no user
  name or password, at most 4 KiB. It runs on insert, `SetBlockKind` and `SetProp(WebAddress)`. A new address clears
  the preview, which belonged to the old one.
- **Preview:** `PageCommand::SetWebBookmarkPreview { id, url, preview }` replaces the whole preview. It does nothing
  if the block is gone, isn't a bookmark, or now holds another address, since the fetch took a while. Its commit has
  the origin `leaf-web-preview`, which the undo manager leaves out, as it does repairs.
- **Reading:** only the address is required. A preview part that is missing or unreadable reads as absent and stays
  stored, so a newer version's value survives (rule 5). A bookmark that lost its address in a merge is repaired into
  a paragraph.
- **Conversions:** into a text kind, the bookmark becomes its address, linked (plain in code). Into a bookmark: an
  empty block, or one whose whole text is one link to the same address; anything else is refused, so no words are
  lost.
- **Empty text container:** a bookmark keeps an empty text container from its creation (`keeps_text_container`), so
  both conversions are plain text edits on a container that already exists. A block without one (a divider) gets it,
  empty, in the step that makes it a bookmark. `PageDoc::text` reads `None` for it, and
  invariant 2 allows it.

## Alternatives considered

- **Create the paragraph's container when converting:** loses the address on redo (the bug above).
- **Two commits grouped as one undo step:** the same container would still be created in a later step than the block.
- **Record previews in the undo history:** undo would then remove a preview the owner never asked for, and the next
  fetch would put it back.

## Consequences

- Undo and redo of every conversion keep the address. The page undo property test now generates bookmarks, address
  changes and previews (old and current address), comparing outlines with previews set aside, since they are never
  undone.
- Only a bookmark whose container a merge removed has none; turning it into text then makes and writes the container
  in one step, the case ADR-007 can't cover either. The property test found the divider case too (divider → bookmark
  → paragraph, undo all, redo all), now covered.
- E15-S03 (paste and menus), E15-S04 (the card) and E15-S05 (the fetcher) build on these commands. Until the card
  exists, a bookmark shows as one muted line with its title or address.
- Plain English: a web bookmark remembers its address and, once fetched, the page's title, description and pictures.
  A fetched preview never becomes an undo step, and turning a bookmark into a line of text and back never loses the
  address.
