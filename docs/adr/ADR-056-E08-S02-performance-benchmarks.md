# ADR-056. Performance benchmarks: divan benches in their own crate, results kept by CI on `main`

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-30
- **Story:** E08-S02

## Context

`PRODUCT_SPEC.md` sets performance budgets: keystroke to paint under 16 ms on a 10,000-block page, and opening a
5,000-block page under 100 ms. The large fixtures from E08-S01 (ADR-055) make those pages. What was missing was a
repeatable way to time the app's own code on them, and a record of the numbers over time.

## Decision

- **A new crate, `leaf-bench`.** Test support only, like `leaf-fixtures`; the app never depends on it, and the
  architecture test checks it never depends on GPUI or `leaf-ui`.
  - Its small library holds the setup the benches share: a big page saved to a scratch workspace folder
    (`SavedPage`), an editor with the caret mid-page (`typing_editor`, `caret_mid_page`), and a page with its view
    model kept apart from an editor (`ViewedPage`). The setup has unit tests, so a bench can't quietly measure an
    empty page or a caret that never landed.
  - `benches/budgets.rs` holds the benches, using `divan` (already chosen in `TECH_STACK.md`).
- **What is measured**, through the same public calls the app makes:
  - `open_page`: load a saved 5,000-block page, start an editor and build the view model.
  - `keystroke`: type one character in the middle of a 10,000-block page. The editor brings its view model up to
    date inside each keystroke, so this includes that update.
  - `view_update`: that view-model update on its own, after a character typed through the model command.
  - `view_rebuild`: a full view-model rebuild of the same page, to show what the incremental update saves.
  - `save`: save one keystroke's change to SQLite; over its 200 samples it includes one compaction (ADR-048).
  - Benches that change the page make the change outside the timing, one iteration per sample.
- **Running them:** `just bench` locally. In CI, a `bench` job runs on every push to `main` (and manual runs, not
  pull requests) and uploads the results table as the `bench-results` artifact.
- **No budget gate in CI.** Shared runners vary too much for a hard limit to be fair; the job records numbers. The
  Phase 0 acceptance pass (E08-S03) checks the budgets on a mid-range laptop.

## Alternatives considered

- **Benches inside `leaf-fixtures` or `leaf-editor`:** fewer crates, but `leaf-editor` must stay wasm-ok and free
  of the store, and the fixtures crate would mix "make documents" with "time the app".
- **`criterion`:** heavier (plotting, more dependencies) and no better for timings this simple.
- **Failing CI on a budget:** catches regressions automatically, but noisy runners would make it flaky. Can be
  revisited with a dedicated runner.
- **Timing in the screenshot tool's `bench` mode:** it measures real frames, but needs a window and a Mac; the
  divan benches run anywhere and isolate the editor and store.

## Consequences

- First numbers on an Apple-silicon Mac: open a 5,000-block page about 49 ms; one keystroke about 0.17 ms; the view
  update after it about 0.01 ms, against about 28 ms for a full rebuild; a save about 0.06 ms.
- A full bench run takes about two minutes, most of it building fixtures and the release build.
- New dependency: `divan` (dev-only, MIT/Apache-2.0), with its small dependency tree (`clap`, `regex-lite`,
  `condtype`).
- Plain English: Leaf now has a stopwatch for the speed promises in the spec. Every change that lands on `main`
  records how long opening, typing and saving take on huge pages, so a slowdown shows up as a number rather than
  a feeling.
