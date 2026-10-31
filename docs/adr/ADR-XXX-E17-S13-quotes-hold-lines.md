# ADR-XXX. A quote holds lines like a callout, and Enter at a block's start leaves the block alone

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E17-S13

## Context

Two Enter rules surprised the owner:

- **Enter at the very start of a heading** split the block there. The empty half before the caret kept the heading
  style, and the text moved into a new paragraph below, so the heading "lost its format".
- **Enter in a quote** always split it into a quote and a paragraph, so a quote could never continue onto a
  second line. Callouts already hold several lines (ADR-070), and the owner said they work well.

## Decision

- **Enter at the start of a paragraph or heading with text** adds an empty paragraph above it. The block keeps
  its kind and the caret stays at its start, as in Notion. Lists, to-dos and action items still split, which
  already keeps their kind on both halves.
- **A quote holds lines, like a callout** (owner's choice, 2026-10-31):
  - Enter is a line break inside the quote, at its start too.
  - Enter on an empty last line, or in an empty quote, removes that line and adds a paragraph below.
  - `Cmd/Ctrl+Enter` leaves from anywhere.
  - The quote's bar runs down all its lines.
- Nothing changes in the data: a quote's text can already hold line breaks, like a callout's, and
  copying writes each line as `> line`.

## Alternatives considered

- **Quotes like lists:** Enter makes a new quote block, and Enter on an empty quote ends it. Each line would be
  its own block, but a quote of several paragraphs would draw as separate bars and move as separate blocks. The
  owner chose the callout behaviour.
- **Fix the start-of-block case in `SplitBlock`** (move the kind with the text): it would change a model command
  that undo, merges and other devices rely on, for a choice that's about the key, not the document.

## Consequences

- Leaving a quote now takes two Enters (or `Cmd/Ctrl+Enter`), like a callout.
- In plain English: pressing Enter at the start of a heading pushes it down with an empty line above, and a quote
  keeps going line after line until you press Enter on an empty line.
