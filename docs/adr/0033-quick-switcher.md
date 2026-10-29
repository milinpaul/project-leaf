# 0033. Quick switcher: a snapshot at open, a small fuzzy matcher, Enter through the dialog

- **Status:** Accepted
- **Type:** Feature
- **Date:** 2026-10-29
- **Story:** E06-S04

## Context

Cmd/Ctrl+P should find a page by part of its title and open it, keyboard only, in under 50 ms with 5,000 pages
(`PRODUCT_SPEC.md`). The full-text search bar (E11-S05, ADR 0019) will extend the same box later, so its state
has to live in `leaf-editor` as plain Rust, like the slash menu (ADR 0026).

## Decision

- **Snapshot at open.** `QuickSwitcher::open` walks the live page tree once, taking each page's title, icon,
  breadcrumb ("Projects / Leaf app") and a lowercased copy for matching. Typing only re-scores that list; it never
  reads the document. Pages in the trash are left out.
- **Own fuzzy matcher (`leaf_editor::fuzzy_match`).** Each query word must match, in any order. Whole title >
  prefix > word start > inside > letters in order (word initials and closeness count). It returns the matched
  byte ranges so the popup shows them in bold. Ties go to the shorter title, then sidebar order. At most 50 rows.
- **Empty query** lists pages opened in this window (most recent first, kept by `WorkspaceEditor`), then the rest.
- **Popup** is a gpui-component dialog near the top of the window. Up/Down are caught before the text field;
  Enter is the dialog's own "OK" action, so the switcher handles it there and closes the dialog only when there
  is a page to open.

## Alternatives considered

- **A fuzzy-matching crate (`nucleo`, `fuzzy-matcher`):** more than titles need, a new dependency, and their
  scores are tuned for file paths; the rules above fit in 150 lines with tests.
- **Re-reading titles from the workspace on each keystroke:** simpler but slower, and it would let a background
  change reorder the list under the pointer.
- **The FTS5 index for titles:** it lags behind unsaved edits and isn't needed until full-text search arrives.

## Consequences

- Opening and typing over 5,000 pages takes about 13 ms in an optimised build (about 55 ms in a debug build).
- Matching ignores case but not accents yet; full-text search (E11-S05) ignores both.
- The recent list resets when the window closes; E06-S08 remembers it across launches.
- Plain English: press Cmd/Ctrl+P, type a few letters of a page's name, and press Enter to jump there. The
  letters you typed are shown in bold, and the pages a page sits inside are shown beside it.
