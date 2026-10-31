# 0071. Page links: one placeholder character with a `page` mark; backlinks in the search index

- **Status:** Accepted (owner accepted the recommended answers, 2026-10-31)
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E10-S01

## Context

Phase 1 adds `[[page links]]` with autocomplete and a backlinks panel (`PRODUCT_SPEC.md`, Phase 1). A link to a page
sits inside a block's text, among ordinary words. That raises these questions:
- how it is stored in the block's Loro text;
- what happens when someone types next to it, formats over it or deletes it;
- how two devices merge edits around it;
- how a version of Leaf from before links treats it;
- what happens when the target page is renamed, trashed or deleted;
- how "which pages link here" is answered fast in a 5,000-page workspace.

The constraints already in place:
- only commands change documents, and undo is per document;
- unknown data from a newer version is never dropped;
- marks this version doesn't know get a style that never expands (`BLOCK_MODEL.md`, "Inline marks");
- the search index is disposable and rebuilt from the documents (ADR 0019).

The spike (`crates/leaf-model/tests/links_spike/`) drove Loro directly to try a model before any production code
exists.

## Decision

**A page link is one placeholder character in the block's text, carrying a `page` mark.**
- The character is U+FFFC, the object replacement character, the standard stand-in for an object inside text.
- The mark's value is the target's `PageId`, and the mark never expands (`ExpandType::None`, like `code` and `link`).
- The title is **not** stored. Drawing looks up the target's current title and icon in the workspace document, so
  renaming a page updates every link to it at once.
- The link is drawn as the target's icon and title, underlined. A plain click opens the page.

```
block text:  "Book the ￼ before Friday"
                       └─ mark page = "01a18f8c-…"   (the ryokan's PageId)
drawn as:    "Book the 🏨 Bookings before Friday"
```

**A target that's gone still shows:**
- in the trash: the title, greyed, with "In trash";
- deleted for good: "Deleted page", greyed.

The link itself is never removed from other pages' text: that would be writing to documents the user isn't editing,
and restoring the page brings every link back to life.

**Backlinks live in the search index** (`search/index.sqlite`) as a new `links` table:
- `source_page`, `source_block`, `target_page`, indexed by `target_page`;
- filled in the same pass that indexes a page's text, so it costs one extra scan of a page already loaded;
- rebuilt with the rest of the index (raising `INDEX_VERSION` rebuilds it once on upgrade);
- like search, it never holds data that isn't in the documents.

## Findings

All of these are tests in the spike (13 tests, plus one measurement).

| Question | Result |
|---|---|
| Typing right before or right after a link | Never joins the link; the link stays one character. |
| Deleting a range that covers a link | Removes it whole; it can't be half deleted. |
| Bold over a sentence containing a link | The link stays a link and is bold too. |
| An older version (no `page` in its mark table) types around a link and italicises nearby text | The link and its page ID survive untouched, both on the older peer and after syncing back. |
| Undo after deleting a link | The same link to the same page comes back; the document is exactly as before. |
| Copy a link and paste it into another page | Points at the same page (the rich clipboard carries marks already). |
| Link deleted on one peer, sentence made bold on the other | Stays deleted; the bold is kept. |
| Link deleted on one peer, words typed next to it on the other | Stays deleted; the words are kept. |
| Link added inside words the other peer deleted | The link survives (inserts are never lost). |
| The same link added at the same place on both peers | Both kept, side by side: visible and one Backspace away, like two people typing the same word. |
| Link pointed at another page on one peer, made bold on the other | Both changes kept. |
| Pointed at two different pages on the two peers | One target wins, the same on both peers, never a mix. |
| **Alternative: today's `link` mark over the title's words** | **Typing inside the title joins the link** (`JaXXpan`), so a link can be half edited, and its words go stale when the page is renamed. |

Measured with 5,000 pages × 20 blocks and 25,000 links, release build, Apple M1 Pro:

| Measure | Result |
|---|---|
| Rebuild: load every page and find every link | about 1 s, almost all of it loading the pages, which rebuilding search does already |
| Find the links in one page after an edit | 0.12 ms |
| Look up one page's backlinks (in memory) | under 1 µs; a SQLite index lookup is well under a millisecond |

## Alternatives considered

- **Today's `link` mark over the title's words, with a `leaf://page/…` address:**
  - older versions would show readable words;
  - but typing inside the words joins the link;
  - the words go stale on rename unless Leaf rewrites every page that links there, which touches documents nobody is
    editing and fights with sync.
- **Literal `[[Title]]` text, matched by title (Obsidian style):**
  - nothing new in the model;
  - but titles aren't unique, renames break links, and two pages with the same name can't be told apart.
- **A link as its own block kind:** links belong inside sentences, not on their own line.
- **Backlinks stored in the documents** (each page keeping a list of who links to it):
  - every link edit would write two documents;
  - merging would need care, and the list could drift from the text;
  - the search index already rebuilds from the documents, so a derived table there can't drift.
- **Backlinks found by scanning all pages when the panel opens:** about 1 s on 5,000 pages, too slow.

## Consequences

- **`leaf-model`:** a `page` mark and `Mark::PageLink(PageId)` (refused in code blocks), with a command that inserts
  the placeholder and its mark in one commit. The placeholder is always marked, and the mark only sits on placeholders.
- **`leaf-editor`:**
  - the caret steps over a link in one key press, and Backspace deletes it whole;
  - the view model resolves each link's title, icon and state (live, in trash, deleted);
  - copying as plain text writes the title, and copying as Markdown writes `[[Title]]`;
  - the `[[` autocomplete is a new popover over the existing fuzzy page matcher (the quick switcher's).
- **`leaf-search`:** the `links` table, extracted alongside the text, and a `backlinks(page)` query.
- **`leaf-ui`:** draws links as an icon and title, opens them on click, shows the autocomplete popover and the
  backlinks panel.
- **Markdown export (E12) writes `[[Title]]`** or a relative link; import turns `[[Title]]` into a link when exactly
  one page has that title.
- **Older versions of Leaf** show a link as a blank placeholder box but keep it, and edit around it safely.

In plain English: a link to another page is stored as a single invisible marker that remembers which page it points
to, not that page's name. Leaf shows the page's current name in its place, so renaming a page updates every link to
it. To show "which pages link here", Leaf keeps a list beside its search index and updates it whenever a page is
saved.

## The owner's answers (2026-10-31)

The owner accepted every recommendation:
1. **Typing `[[` starts a link,** and the slash menu gets a "Link to page" entry. `@` is not a trigger.
2. **When no page matches,** the popover offers "New page '…'", which creates the page inside the current page and
   links it.
3. **Backlinks show as a "3 backlinks" line under the page title,** hidden when there are none, which opens a list. Each
   entry shows the page, its icon and the sentence with the link, and clicking it jumps there.
4. **Links to a block inside a page come later** (E10-S08, P2). This design extends to them with a mark value that
   adds a block ID.
5. **Unlinked mentions come later** (E10-S09, P2).
