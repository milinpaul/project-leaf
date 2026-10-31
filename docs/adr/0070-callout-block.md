# 0070. Callout block: one text with lines, and a tone stored by name

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-31
- **Story:** E21-S01

## Context

The owner asked for callouts in Phase 0 (the spec had them in Phase 2): a tinted box with an icon that makes text
stand out, in five tones (Info, Success, Warning, Error, Note), a fixed icon per tone, text only. Two questions:
how a callout holds several lines, and how its tone is stored so newer tones don't break older versions.

## Decision

- **`BlockKind::Callout { tone: CalloutTone }`**, type `callout`, with rich text (marks allowed) and no children.
- **Lines live in its one text**, separated by `\n`, as in code blocks and table cells. `Enter` adds a line break;
  `Enter` on an empty last line, or in an empty callout, removes that line and adds a paragraph after it;
  `Cmd/Ctrl+Enter` leaves from anywhere. `Backspace` at the start turns it into a paragraph, like other kinds.
- **The tone is a prop, `tone`, stored by name** (`info`, `success`, `warning`, `error`, `note`) and changed with
  `SetProp(BlockProp::CalloutTone)`, one undo step. A missing tone reads as Info. A name this version doesn't know
  reads as `CalloutTone::Other(name)` and is written back unchanged (rule 5); the UI draws it like a note. An
  empty name can't be written.
- **It is a "Turn into" kind** (after Quote), so any text block can become one and back, keeping its text and marks.
  A new callout starts as Info.
- Copying as Markdown writes GitHub's alert syntax (`> [!NOTE]` …); Leaf's own clipboard fragment carries the tone
  by name. Pasting alerts back, and the slash menu, come in E21-S03.

## Alternatives considered

- **Child blocks inside a callout** (as Notion allows): richer, but the owner chose text only, and nested blocks
  would bring indent and drop rules for a container kind. Can be added later; one text converts cleanly.
- **Tone as an enum that fails on unknown names:** a newer version's tone would make the block unreadable here.
- **One kind per tone** (`info_callout`, …): turning between tones would be a kind change, and every list of kinds
  would grow five entries.

## Consequences

- Callouts work with search, undo, copy and paste, and saving with no special cases; drawing them is E21-S02.
- Plain English: a callout is a highlighted box of text. It remembers its colour by name, so a colour added in a
  later version of Leaf survives being opened and edited in this one.
