# ADR-XXX. Links to a block: a page link with a `block` mark

- **Status:** Accepted (owner chose the recommended answers, 2026-11-01)
- **Type:** Architecture
- **Date:** 2026-11-01
- **Story:** E10-S08

## Context

ADR-071 made a page link one placeholder character with a `page` mark, and left links to one block of a page for
later, "with a mark value that adds a block ID". This story builds them. The questions were:
- how the target is stored;
- how a link is made;
- what it shows, live and when the block is gone;
- where a click lands.

The block's words live in the target page's document, not in the workspace, so drawing one needs that page in memory.

## Decision

**Stored as a second mark on the same placeholder.**
- The `page` mark is unchanged; a `block` mark beside it holds the block's `BlockId` (`PageCommand::InsertBlockLink`).
- A version of Leaf with page links but not block links sees an ordinary link to the page and keeps the `block` mark
  (unknown marks never expand and are never dropped).
- Putting both IDs into the `page` value would have turned these links into "Deleted page" there.
- The mark goes on placeholder by placeholder, like `page` (ADR-071's note on E10-S02).
- Invariant 10 grows: a `block` mark sits only on a placeholder with a `page` mark, and the merge repair puts that
  back. Re-pointing a link at another page drops its block.

**The owner's answers (2026-11-01):**
1. **Label:** the page's icon and title, then the block's first words: "🏨 Bookings › Book the ryokan…". At most 30
   characters, cut at a word break. An empty block shows the page alone.
2. **Making one:** "Copy link to block", last in the ⋮⋮ menu, puts a fragment holding the link on the clipboard.
   - Pasting it into any page makes the link.
   - Other apps get `[[Bookings › Book the ryokan]]` as Markdown, or the same words as plain text.
   - Searching blocks from the `[[` menu is not part of this.
3. **Block gone:** "🏨 Bookings › deleted block", greyed (`LinkState::BlockGone`). A click still opens the page, at the
   top. A page in the trash or deleted shows as a page link does.
4. **Arriving:** a click goes through `Shell::open_at`, the path search results and backlinks already use: the block
   is scrolled to, the caret goes to its start, and the block is tinted briefly.

**Live words.**
- Opening a page also loads every page its block links point at (`WorkspaceEditor::block_link_pages_to_load`).
- The label reads the block's words from the loaded page when it is drawn, so edits there show at once.
- Until a page is loaded, its links show the page alone.

**Backlinks.** A block link is still a link to the page, so the backlinks list counts it with no change to the index.

## Alternatives considered

- **One mark whose value is "page#block":** a single mark to keep in step. But older versions couldn't read the page
  and would show "Deleted page".
- **Keeping a copy of the block's words in the link:** nothing to load. But the words go stale as soon as the block is
  edited, and the owner wanted them current.
- **Reading the words from the search index:** no page to load. But the index is a moment behind, it leaves out pages
  in the trash, and the editor would depend on the index's answers to draw text.
- **Picking a block from the `[[` menu:** needs a search across every block of every page. It can come later.

## Consequences

- **More pages in memory:** a page with block links loads their target pages, usually a handful, once per session.
  Pages loaded this way aren't edits, so they aren't saved.
- **Older versions** show a block link as a link to its page and open it at the top.
- **Markdown export (E12)** can name block links the way the clipboard does.

In plain English: a link to a paragraph is the same invisible marker as a link to a page, plus a note of which
paragraph. Leaf shows the page's name and the paragraph's first words. Clicking it opens the page at that paragraph
and highlights it. If the paragraph has been deleted, the link says so and opens the page at the top.
