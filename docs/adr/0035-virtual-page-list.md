# 0035. The page is a virtual list; rows are diffed to keep it in step

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E07-S01

## Context

The page area drew every block of the open page, each with its own text input, in a column that didn't scroll.
A 10,000-block page (`PRODUCT_SPEC.md` asks for large pages to stay smooth) would lay out ten thousand inputs every
frame. Blocks differ in height (headings, wrapped paragraphs, code), so a fixed-height list doesn't fit.

## Decision

- **GPUI's `list` with a `ListState`**: a variable-height virtual list that lays out and draws only the items on
  screen, plus 600 px of overdraw so fast scrolling shows no gaps.
  - Item 0 is the page title and the blocks follow, so the title scrolls away with the page, as in Notion.
  - Each item sits in the centred reading column.
  - The "Turn into" toolbar stays above the list.
- **Only visible rows are built**, so text inputs exist only for them. Inputs of deleted blocks are dropped.
- **Keeping heights right**: the list remembers measured heights by position. Each frame, `page_list::changes`
  compares the rows drawn last time with this frame's:
  - rows whose block stayed but whose content changed are re-measured in place;
  - the run of rows between the unchanged start and end (added, removed or moved blocks) is spliced.

  This keeps the scroll position, so typing or pressing Enter doesn't make the page jump. Opening another page
  resets the list to the top. The comparison is linear in the number of rows: about a millisecond for 10,000.
- **Focus follows:** moving the keyboard to a block that is scrolled away (arrows, Enter at the bottom) scrolls it
  into view.
  A block already on screen never moves the page (amended in E17-S06): `PageList::reveal` checks the row against the
  view first, because GPUI's `scroll_to_reveal_item` lines a row up with the bottom even when it is visible, and with
  the top of the window inside a tall first item (a cover above the title) that scrolled to the very top.
- **Scrolling** (amended in E17-S01/S02, ADR 0061): mouse-wheel steps glide instead of jumping, and dragging a
  text selection past the top or bottom edge scrolls the page.
- **Markers by depth**, as in Notion: bullets • ◦ ▪ and numbers 1. a. i., repeating every three levels.

- **Checked with real rendering:** `just screenshot` (`leaf-ui/examples/screenshot.rs`) renders a workspace in an
  off-screen window through GPUI's visual test context and the real Metal renderer, and saves a PNG. It works with the
  screen locked. With `bench` it times frames. On a 10,000-block page in a release build, scrolling a line per frame
  takes about 2.9 ms per frame (slowest 7.1 ms), and jumping across the page 6.7 ms (slowest 8.0 ms). That is well
  within the 16 ms of a 60 fps frame; a debug build stays around 8 ms.

## Alternatives considered

- **`uniform_list`:** simpler and faster, but every row the same height. Headings and wrapped text rule it out.
- **Reset the list on every change:** correct heights, but scroll position and measurements are lost on each
  keystroke.
- **Have `PageView` report which rows changed:** precise, but it would tie the core view model to the list's
  index-based API. Diffing two row vectors in the UI is simple and cheap enough.

## Consequences

- The rich text element (E07-S02) replaces the inputs inside rows; the list stays.
- `Shell::scroll_page_to(block)` scrolls to a block even right after opening the page. Search results (E11) can use
  it to show the matching block.
- Inputs created while scrolling stay alive until their block is deleted. That is fine for interim inputs; the rich
  text element holds no per-row state.
- Plain English: the page now scrolls, and only the part you can see is drawn, so even a page with ten thousand
  blocks scrolls smoothly. Nested lists show different bullets and numbering at each level.
