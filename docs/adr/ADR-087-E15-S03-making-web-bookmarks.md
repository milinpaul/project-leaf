# ADR-087. Making web bookmarks: an offer after a paste, and an address box from the slash menu

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E15-S03

## Context

ADR-083 added the web bookmark block, but there was no way to make one. The story asks for three ways: pasting a lone
web address offers to make it a bookmark (a link stays the default), the slash menu gets "Web bookmark", and the block
menu swaps a bookmark and a link. A pasted address already becomes a link (E17-S10), and Notion-like editors show a
small menu after such a paste.

## Decision

- **The offer:** pasting a lone `http`/`https` address (not over selected text) goes in as a link, as before. The editor
  then remembers an offer: an anchor at the link's start, its length and its address. It stands only while the caret
  sits right after the untouched link. Typing, moving, clicking elsewhere, Escape or any key other than Up, Down and
  Enter ends it, so nothing ever has to close it by hand. The menu has "Dismiss" (highlighted first, so Enter keeps the
  link) and "Create web bookmark".
- **Creating from the offer:** a block that is only the link becomes the bookmark (the model accepts a block whose
  whole text is one link to the address, ADR-083), and the caret goes to a new line below. Otherwise the link's words go
  and the bookmark goes below the block, or below the table from a cell. This is one undo step of its own, so undo gives
  the link back.
- **The slash menu:** "Web bookmark" (also found by "bookmark", "url", "website") removes the `/query` and asks for an
  address the way "Image" asks for a file: the editor keeps the place as an anchor, and the UI shows a box under it.
  `leaf.app/blog` gets `https://` (`openable_url`), and anything a bookmark can't hold keeps the box open with a hint.
  The bookmark replaces an empty line, with the caret on a new line below, or goes below the block.
- **The block menu:** "Turn into link" on a bookmark, and "Turn into web bookmark" on a block that is only one link to a
  web address. "Turn into" a bookmark over a selection skips blocks that would lose words.
- **Pasting whole blocks:** a copied divider, image or web bookmark pasted into a line with text goes below the line,
  like a table. Before, its empty text joined the line and the block was lost; the story's tests found this.

## Alternatives considered

- **Always make a bookmark from a pasted address:** surprising in running text, and it needs the network to look
  right. A link stays the default, as the epic agreed.
- **An offer that closes on a timer, or stays until dismissed:** a timer races the owner, and a menu that stays would
  need its own focus. Tying it to the caret position needs no bookkeeping beyond the anchor.
- **Ask for the address inside a placeholder block:** the model needs an address to create a bookmark, so a block
  without one would be an invalid state to keep in the document.

## Consequences

- E15-S04 draws the card in place of the placeholder line. Until then, a bookmark shows as "Web bookmark: <address>".
- Pasting an address followed by Enter now dismisses the offer instead of starting a new line, as in Notion; a second
  Enter starts the line.
- Plain English: paste a web address and Leaf offers to turn it into a web bookmark card, or keep it as a link; or type
  `/bookmark` and give the address. Undo takes either back in one step.
