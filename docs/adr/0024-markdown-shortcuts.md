# 0024. Markdown shortcuts fire on typing and undo separately

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S01

## Context

`PRODUCT_SPEC.md` lists Markdown shortcuts at the start of a block: `# `, `## `, `### `, `- `, `* `, `1. `, `[] `,
`> `, ```` ``` ````, `---`. Undo right after a conversion must give back the typed characters, the usual escape
when the user meant them literally.

Typing commits merge into one undo step when they come within the merge interval (~500 ms). A conversion right after
typing `# ` would land in the same step, and undo would remove both.

## Decision

- **When it fires:** after every `insert_text` (typing, an input-method commit, typing over a selection), the editor
  checks whether everything before the caret is exactly a shortcut. It fires only at the start of a block, by
  construction. Text after the caret stays and becomes the converted block's text.
  - Any number works for numbered lists (`3. `), since numbers are worked out when drawing.
  - `---` only fires on an otherwise empty block, since a divider holds no text. The caret goes to a new paragraph
    below the divider.
  - Code blocks and image captions are literal, so no shortcuts fire there. A block that already has the asked-for
    kind keeps the characters as text. Switching kinds works, for example `## ` in a heading 1, or `[] ` in a list
    item.
- **Undo:** the conversion (delete the typed prefix, change the kind, maybe add a paragraph) is one undo step of its
  own, via new `PageDoc::as_separate_step`.
  - It turns the merge interval off for the step's first commit, so it doesn't join the typing before.
  - It keeps it off until the next commit, so the typing after doesn't join the conversion. Then the interval comes
    back.
  - Undo right after gives back the paragraph with `# `, with the caret after it. Redo converts again.

## Alternatives considered

- **Convert in the same step as the typing:** simplest, but undo would also remove the typed `#`, and the user couldn't
  get the literal text back.
- **Check on a separate "space pressed" key event:** misses ```` ``` ```` and `---`, which end without a space, and
  input methods that commit text in one go.
- **Add a "start new undo step" call to the model:** works, but leaves the caller to remember to end it.
  `as_separate_step` keeps both ends in one place.

## Consequences

- Pasted Markdown is a separate story (E05-S06); these shortcuts only react to typing.
- Plain English: typing `# ` at the start of a line makes it a heading, `- ` a bullet, `[] ` a to-do, and so on. If
  that wasn't what you wanted, one Undo turns it back into the characters you typed.
