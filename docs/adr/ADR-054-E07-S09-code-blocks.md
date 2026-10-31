# ADR-054. Code blocks: a language picker storing fence names, Tab indents lines with four spaces

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E07-S09

## Context

Code blocks are drawn in a monospace font on a tinted background (E07-S02), and they already store a `language`
(BLOCK_MODEL, ADR-007), but nothing on screen showed or changed it. In code, Tab nested the block like any other,
so there was no way to indent a line of code. The story asks for a language picker, and for Tab to insert spaces
inside code.

## Decision

- **A language is stored as a Markdown fence name** (`rust`, `python`, `cpp`), the same value pasting fenced code
  already stores and copying writes back out. `""` means plain text.
- `leaf_editor::CODE_LANGUAGES` lists 26 common languages plus Plain text, each with a label.
- A stored name Leaf doesn't list (from a pasted fence or a newer Leaf) is kept and shown as it is.
- `set_code_language` is one undo step, and setting the same language again adds none.
- **The label** sits at the code block's top right, shown while the pointer is over the block. A click opens a
  searchable list:
  - the block's current language is first and ticked;
  - typing filters with the same fuzzy match as the slash menu and quick switcher;
  - Enter takes the top match, a click takes any, and Escape or a click elsewhere closes the list without a change.
- **Tab inside a code block indents with four spaces** (`CODE_INDENT`):
  - with just a caret, Tab inserts four spaces there;
  - with a selection inside the block, Tab indents every line it touches;
  - Shift+Tab removes up to four leading spaces from the caret's line, or from each selected line;
  - a selection ending at the very start of a line doesn't touch that line, as in code editors;
  - the caret and selection move with the text, and each press is one undo step.
- A selection across blocks, or whole selected blocks, still nest or un-nest as before. That is how to nest a code
  block; dragging it by its handle also works.

## Alternatives considered

- **Store display names ("C++"):** fenced code would no longer round-trip, and names would depend on Leaf's spelling.
- **Two spaces, or a tab character:** four spaces is the most common default for the languages listed (Rust, Python,
  Java, C). A tab character renders at an unpredictable width. The width is one constant if the owner wants another.
- **A dropdown always visible on every code block:** it is noisy on pages with many snippets. Showing it on hover
  matches the ⋮⋮ handle.

## Consequences

- Syntax highlighting can use the stored language later. The spec calls it optional, and it isn't done here.
- In the list, arrow keys don't move the highlight yet; typing narrows the list and Enter takes the top match.
- Plain English: hover over a code block to see its language in the corner; click it to pick another, and type to
  find it quickly. Inside code, Tab now indents the line by four spaces and Shift+Tab takes them away, like a code
  editor, instead of tucking the whole block under the one above.
