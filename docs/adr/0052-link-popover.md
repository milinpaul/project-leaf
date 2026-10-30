# 0052. Link popover: an address box under the selection; only web and mail addresses are opened

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S07

## Context

Links could be stored and drawn (E04-S07, E07-S02), but the owner had no way to add or follow one: `Cmd/Ctrl+K` over
selected text did nothing (search takes it without a selection, E11-S05). The story asks for a popover to edit a link's
address, and `Cmd/Ctrl+click` to open a link. Opening hands an address to the operating system, and pasted text can
carry any address, so which ones Leaf opens needed deciding.

## Decision

- **The popover** is a small view (`LinkPopover`) floating under the start of the selection. It holds:
  - an address field, filled in and selected when the whole selection is already one link, so typing replaces it;
  - Open, enabled for an address Leaf will open;
  - Remove, shown when there is a link to remove.
- **Enter** saves through `PageEditor::set_link` (a blank address removes the link). **Escape** or a click outside
  closes it without changes. Either way the page gets the keyboard back with the selection as it was. The editor's
  existing `set_link` / `remove_link` / `active_marks` do the work, as one undo step.
- **`Cmd/Ctrl+click`** on linked text (`PageEditor::link_at`: the character after the click, or the one before) opens
  it with `cx.open_url` and leaves the caret where it was. A plain click still places the caret, so links stay easy
  to edit.
- **Only `http`, `https` and `mailto` are opened** (`leaf_editor::openable_url`). An address with no scheme that looks
  like a web address (`example.com/page`) gets `https://`. Anything else, such as `javascript:`, `file:`, another
  app's scheme or a bare word, is stored as typed but never opened, so a pasted link can't run something on the
  owner's computer.
- **Pasting an address** (amended in E17-S10): a lone `http(s)://`, `mailto:` or `www.` address pasted from
  outside Leaf becomes a link whose text is the address, or, over selected text, links that text
  (`leaf_editor::pasted_link`, then `set_link`). It is stricter than `openable_url` so a pasted `notes.txt` stays
  text; several words, or a paste into a code block, stay text too. A preview card is bookmarks (E15, Phase 1).
- Opening a link only asks the system's browser or mail app to show it; Leaf itself makes no network request, so
  Phase 0's no-network rule holds.

## Alternatives considered

- **A plain click opens links:** that is how web pages work, but in an editor it makes linked text hard to click into
  and edit. Notion and most editors use a modifier or a hover card.
- **Open any address the system can handle:** it is convenient for app links (`slack://`), but it is a risk for text
  pasted from untrusted places. It can be widened later, with a confirmation.
- **Store the `https://` that's added to a bare address:** this would change what the owner typed. Adding it only
  when opening keeps the stored address as written.

## Consequences

- Window tests cover adding, changing and removing a link, Escape and a click outside, and Cmd/Ctrl+click opening a
  link while a plain click places the caret.
- A link popover on hover (showing the address without editing) is not part of this story.
- Plain English: select some words and press Cmd+K (Ctrl+K on Windows and Linux). A box appears under them: type or
  paste a web address and press Enter to make the words a link. Do the same on a link to change it, or press the bin
  to remove it. Cmd-click (Ctrl-click) a link to open it in your browser. Leaf only opens web and email links.
