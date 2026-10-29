# 0055. Large test fixtures: a small crate, built through real commands, fixed by a seed

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E08-S01

## Context

The performance budgets in `PRODUCT_SPEC.md` are measured on big documents: a 10,000-block page, and a
5,000-page workspace (with about 100,000 blocks for search). Benchmarks (E08-S02), the search budget (E11-S07) and
the acceptance pass (E08-S03) all need the same big documents, built the same way each time. Until now each test
grew its own ad hoc loop of titles like `"block 42"`.

## Decision

- **A new crate, `leaf-fixtures`.**
  - Test support only: crates use it as a dev-dependency and the app never does.
  - It depends on `leaf-core`, `leaf-model` and `leaf-store`, and like the core crates never on GPUI or `leaf-ui`
    (the architecture test checks it).
- **Built through the real model commands:** `InsertBlock`, `SetMark`, `CreatePage`, `SetPageIcon`. A fixture is
  an ordinary document that saves, loads, indexes and draws like one a person made, and it can't hold anything
  the model would refuse.
- **What it makes:**
  - `big_page(seed, blocks)` / `fill_page`: headed sections of paragraphs, bulleted, numbered and to-do lists
    nested up to three levels, quotes, code and dividers, with about a third of text blocks formatted (bold,
    italic, strike, inline code or a link).
  - `page_tree(seed, pages)`: a page tree up to six levels deep with titles and some emoji icons.
  - `big_workspace(seed, pages, blocks_per_page)`: the tree with every page filled.
  - `save_to_folder(path, fixture)`: writes a real workspace folder.
  - The full sizes are named `BIG_PAGE_BLOCKS` (10,000), `BIG_WORKSPACE_PAGES` (5,000) and `BLOCKS_PER_PAGE` (20,
    so 100,000 blocks).
  - `just fixture <folder> [seed]` writes the full-size workspace for opening in Leaf.
- **A seed fixes the outline, not the IDs.**
  - A small SplitMix64 generator, in the crate and with no dependency, drives every choice.
  - The same seed gives the same page tree, titles, icons, block kinds, nesting, text and formatting on every
    platform.
  - Page and block IDs are minted by the model (UUID v7) and differ between runs; tests and benchmarks must not
    depend on them.
- **Text** comes from a fixed list of everyday words, with a few accented and non-Latin ones, so search and layout
  meet them.

## Alternatives considered

- **A `fixtures` feature on `leaf-model`:** it would keep the crate count down, but saving to a folder needs
  `leaf-store`, which `leaf-model` must not depend on, and test-only code would sit in a wasm-ok core crate.
- **Writing Loro data directly, or a stored sample file:** faster to load, but it could drift from what commands
  produce, and a binary file can't be reviewed.
- **The `rand` crate:** a dependency for a 20-line generator, and its streams can change between versions, which
  would silently change every fixture.
- **Deterministic IDs:** the model mints IDs itself, and changing that for tests would weaken a real invariant.

## Consequences

- Building the full 100,000-block workspace takes about 6 s in a debug build and 3 s optimised; the 10,000-block
  page and the 5,000-page tree take under a second. The saved workspace is about 31 MB.
- Changing the word list or the generator changes every fixture. A test pins the generator's first output so such
  a change is deliberate.
- Plain English: Leaf can now make huge practice notebooks on demand (thousands of pages, a hundred thousand lines
  of notes), the same every time, so speed can be measured fairly and slowdowns are noticed.
