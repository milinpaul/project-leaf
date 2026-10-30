# 0030. Internal clipboard: versioned JSON fragments; pasting blocks mid-text splits the line

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E05-S05

## Context

Copy, cut and paste must carry rich content inside Leaf: formatting, block kinds and nesting. The content passes
through the system clipboard, so it needs a serialized form that a later Leaf can still read or safely refuse. Pasting
blocks into the middle of a line must split it sensibly, and a copy pasted back must equal the original.
`leaf-editor` is a core, wasm-ok crate with no access to the system clipboard.

## Decision

- **`Fragment`**: either `Text(spans)`, formatted text from inside one block, or `Blocks(trees)`, whole blocks with
  their kind, formatted text and children.
  - **Copy:** a selection in one block becomes `Text`. A selection across blocks becomes `Blocks`, trimmed at both
    ends and nested as the blocks are relative to each other. Selected blocks are copied whole.
  - **A whole block** (amended in E17-S05): all of one block's text selected, or just the caret in a block with
    nothing selected, copies that block as `Blocks` (its kind and text, not what is nested in it), so a heading
    or to-do keeps its kind. A table cell copies as `Text`: cells only exist inside their table. Cut with only the caret does nothing. Pasting blocks lets the first take a line's kind
    only when that line is empty; at the start of a line with text, the text joins it.
  - **Cut:** copy, then delete, as one undo step.
- **Wire format:** JSON, versioned:
  `{"leaf_fragment": 1, "text": [...]}` or `{"leaf_fragment": 1, "blocks": [...]}`.
  - Blocks carry `"type"` with the document's type names plus their properties. Text runs carry `"insert"` plus only
    the marks that are set.
  - A reader refuses other versions, both `text` and `blocks` together, or unknown block types: `from_json` returns
    `None`, and the UI pastes plain text instead (E05-S06).
  - Blocks from newer versions can't be created here, so they are left out of a copy.
  - `serde`/`serde_json` are already workspace dependencies and wasm-ok; `leaf-editor` now uses them too.
- **The editor never touches the system clipboard.** The UI writes `to_json()` under a Leaf-specific type and passes
  back what it reads.
- **Paste** (one undo step, replacing any selection; the caret ends after the pasted text):
  - **Text:** inserted at the caret with exactly its own marks. Neighbouring formatting doesn't leak in, as it would
    when typing. Code blocks get plain text.
  - **Blocks mid-text:** the line splits at the caret. The first block's text joins the text before the caret, the
    other blocks follow (with their children), and the rest of the line goes after the last pasted text.
  - **Blocks into an empty paragraph:** the first block takes its place, kind included. So copying blocks and pasting
    them on an empty line gives back exactly what was copied.
  - **Special cases:**
    - A caption takes no blocks in: they go after the image.
    - With whole blocks selected, the paste goes after the last of them.
    - If the last pasted block holds no text (a divider), the rest of the line goes into a new paragraph.
    - Children under a kind that can't hold them follow it instead.

## Alternatives considered

- **Copy as Markdown only:** loses kinds Markdown can't express (to-do state, code language, image width) and needs a
  parser just to paste within Leaf. Markdown and plain text are written alongside for other apps in E05-S06.
- **Copy the raw Loro data:** would also carry data from newer versions, but ties the clipboard to the storage
  layout. A paste into an older Leaf would then fail opaquely instead of being refused by version.
- **Always insert pasted blocks below the line:** simpler, but pasting in the middle of a sentence would leave the
  sentence's end above the pasted blocks.

## Consequences

- Formatting and properties from newer versions aren't copied, because the fragment holds what this version knows.
  The document itself still never loses them: copying never writes, and duplicating (ADR 0028) copies everything.
- Plain English: copy, cut and paste work inside Leaf with formatting, block types and indentation kept. Pasting
  blocks in the middle of a line splits it around them, and one Undo takes a paste back.
