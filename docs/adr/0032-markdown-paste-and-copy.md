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

- Markdown features without blocks (footnotes, HTML, images by URL) paste as text. Images by file come with
  E05-S07. Tables paste as tables since E16-S04 (below).
- Markdown export of whole pages (Phase 1) can reuse the writer.
- Plain English: paste text from anywhere and it becomes proper blocks. Each line is a paragraph, `# Title` becomes a
  heading, `- item` a bullet, `- [x]` a ticked to-do, `**bold**` bold, and so on. Copying from Leaf also gives other
  apps clean Markdown and plain text.

## Update 2026-10-30 (E16-S04): tables

- **Pasting:** a GitHub-style table (header line, `|---|` delimiter line with as many columns, rows) becomes a table
  block with a header row (`clipboard/markdown/table.rs`).
  - Outer pipes are optional, `\|` is a pipe in a cell, `<br>` a line break, and cells keep their inline formatting.
  - Short rows get empty cells; extra cells are dropped (GFM's rule).
  - Text copied from a rendered page often has blank lines between rows, so a row after blank lines still belongs to
    the table when it starts with `|`. A pipe-less line after a blank line ends the table, so prose with a `|` in it
    stays prose. A line with `|` but no delimiter line under it is never a table.
- **Copying:** a table writes a Markdown table (first row as the header, which GFM requires; pipes escaped, line breaks
  as `<br>`) and plain text as a line per row with cells separated by tabs, which spreadsheets read as cells.
  Leaf's own fragment carries `table`, `table_row` and `table_cell` blocks (an additive change, so
  `FRAGMENT_VERSION` stays 1; an older Leaf can't read the fragment and falls back to the Markdown).
  Selected cells copy as a table of just those cells.
- **Pasting a table** builds it with the table commands and fills each cell. It can't join a line's text, so it goes
  below the line (or replaces an empty paragraph) and the text after the caret moves to a new line below it. Into a
  cell, a table's cells go in as lines of text: tables never nest.

## Update 2026-10-31 (E19-S04): action items

- **Copying:** an action item writes a task list line with its fields in a group at the end, in plain words any app
  shows readably: `- [ ] Send invoice (due 2026-10-31 14:00, high)`. The date is `YYYY-MM-DD` and the time 24-hour
  `HH:MM`, as stored, so nothing depends on language or time zone. An item with neither a date nor a priority ends in
  `(action item)`, so it doesn't come back as a to-do (`clipboard/markdown/task.rs`).
- **Pasting:** a to-do line becomes an action item only when its last `(…)` group is made entirely of those parts
  (`due` with a date and an optional time, `high` / `medium` / `low`, `action item`; any order and case, each at most
  once). Anything else in brackets, like `- [ ] Call Sam (after lunch)`, stays to-do text.
- **A to-do whose text happens to end like that** (`Pick a level (high)`) has its bracket escaped when copied
  (`\(high)`), so it pastes back as a to-do with the same text.
- **Alternatives considered:** Obsidian Tasks' emoji (`📅 2026-10-31 ⏫`) is compact but unreadable without the
  plugin and awkward to type; a `[!]` box (`- [!] ...`) is not a GitHub task list and shows as a bullet elsewhere.
- Plain English: copy an action item into an email or notes app and it reads naturally, with its date and priority in
  brackets; paste that back into Leaf and it is an action item again.
