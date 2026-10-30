# 0034. Page icon picker: the `emojis` crate, Unicode 13 and older, in a popover

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S05

## Context

A page has an optional emoji icon (`PRODUCT_SPEC.md`), shown above its title and in the sidebar. E06-S05 asks for an
emoji picker with search, like Notion's. That needs the emoji list with names and groups, which Leaf did not have.

## Decision

- **Data from the `emojis` crate** (MIT/Apache-2.0 and Unicode-3.0, no required dependencies, pure data, so
  `leaf-editor` stays wasm-ok). It brings Unicode's emoji list, names and groups, and GitHub's shortcodes (`:+1:`).
- **Only emoji from Unicode 13 or older**, without skin-tone variants: every supported OS font can draw them, and
  the grid stays calm. About 1,500 emoji in nine sections.
- **Search** (`leaf_editor::EmojiPicker`): each typed word must start a word of the emoji's name, a shortcode or its
  group's title ("food" finds the whole group, with emoji named for it first). Enter picks the highlighted emoji:
  the first result, unless the arrow keys (amended in E17-S04: Left/Right by one, Up/Down by a row, across
  sections, scrolled into view) or the mouse moved the highlight. The picker catches the arrows before its search
  field, which would otherwise move its caret. A
  Random button picks from the friendlier groups using a seed the UI passes (the editor never reads the clock).
- **UI**: the icon sits above the title (an "Add icon" button appears on hover when there is none). Clicking it opens
  a popover under it: search field, Random, Remove, and a grid that draws only the rows on screen.
- `WorkspaceEditor::set_icon` writes `SetPageIcon` and skips unchanged icons, so picking the same emoji leaves no
  undo step.

## Alternatives considered

- **A hand-written list of a few hundred emoji:** smaller, but always missing the one the owner wants, and
  names/keywords would be ours to maintain.
- **Newer emoji (Unicode 14+):** they show as empty boxes on older systems, and a page icon must look the same on
  every device the workspace is opened on.
- **A dialog instead of a popover:** simpler to test, but Notion's picker sits under the icon, which feels lighter.

## Consequences

- One small dependency (about 1 MB of source, mostly tables).
- On Linux, GPUI's text stack prefers `DejaVu Sans` over `Noto Color Emoji` for the basic smileys, so they draw in
  black and white where DejaVu is installed; macOS and Windows are fine. Recorded as E08-S06.
- Plain English: hover over a page title and choose "Add icon", type a word like "rocket" or "food", and click an
  emoji (or press Enter). Click the icon again to change or remove it.
