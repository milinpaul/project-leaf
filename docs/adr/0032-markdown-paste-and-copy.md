# 0032. Pasting Markdown and plain text; copying writes Markdown and plain text

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S06

## Context

Text pasted from other apps (notes, chat, editors, `.md` files) should become blocks:
- each line a paragraph (`PRODUCT_SPEC.md`: "Paste multi-line text");
- Markdown syntax converted.

Copying from Leaf should give other apps plain text and Markdown alongside Leaf's own fragment (ADR 0030).
`BLOCK_MODEL.md` already defines a Markdown mapping for export.

## Decision

- **A small Markdown reader in `leaf-editor`** (`clipboard/markdown.rs`, `markdown/inline.rs`), not a crate.
  - **Blocks**, line by line, by the mapping table:
    - `#`–`######` headings; deeper than 3 become heading 3;
    - `-`/`*`/`+` bullets, `1.`/`1)` numbers;
    - `- [ ]`/`- [x]`/`[]` to-dos;
    - `>` quotes;
    - fenced code with its language (a longer fence can hold a shorter one; an unclosed fence runs to the end);
    - `---`/`***`/`___` dividers.
  - **Nesting:** indentation nests, two spaces (or a tab) a level.
  - **Plain lines:** every other line is a paragraph of its own. Blank lines only separate.
  - **Wrapped list items:** an indented line under a list item or quote that has no marker of its own continues that
    item.
  - **Inline**, following CommonMark where pasted text needs it:
    - `**bold**`, `*italic*`/`_italic_`, `~~strike~~`;
    - `` `code` ``: a span closes only at an equal backtick run, and an unmatched run is plain text;
    - `[text](url)`: the text ends at its first `]`, which must be followed by `(`;
    - `<url>`;
    - backslash escapes;
    - `_` only at word edges, so `snake_case` stays.
- **`paste_text(text)`:**
  - one line without block syntax pastes into the line, with its inline formatting;
  - anything else pastes as blocks through `paste` (one undo step, the line split around them);
  - into a code block, text goes in exactly as it is.
- **The UI reads the clipboard in order:** Leaf's fragment (ADR 0030), then text through `paste_text`.
- **On copy**, the UI also writes `Fragment::to_markdown()` and `to_plain_text()`.
  - Markdown follows the mapping table: blank lines between top-level blocks except within a list, lists numbered by
    position, children indented two spaces, `_italic_` so bold+italic reads back.
  - Escaping covers only what could read as formatting, so `depends_on: [E04-S05]` stays readable.
  - Writing then reading gives back the same blocks for everything the mapping covers.
- **Tested on a real backlog story** (`story-E05-S01.md`), with an `insta` snapshot of the blocks it becomes. Copied back
  out, the story matches the original except that wrapped lines are joined and a lone ```` ``` ```` is escaped.

## Alternatives considered

- **`pulldown-cmark` or `comrak`:** complete CommonMark, but a new dependency for the core crate. They also join
  consecutive lines into one paragraph (CommonMark's rule), which is wrong for pasted plain text, and their output
  would still need mapping onto our blocks. Paste only needs the subset the block model has.
- **Only plain text on paste:** loses headings, lists and links from every Markdown source, the common case for notes
  copied from editors and chats.

## Consequences

- Markdown features without blocks (tables, footnotes, HTML, images by URL) paste as text. Images by file come with
  E05-S07.
- Markdown export of whole pages (Phase 1) can reuse the writer.
- Plain English: paste text from anywhere and it becomes proper blocks. Each line is a paragraph, `# Title` becomes a
  heading, `- item` a bullet, `- [x]` a ticked to-do, `**bold**` bold, and so on. Copying from Leaf also gives other
  apps clean Markdown and plain text.
