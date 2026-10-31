# Contributing to Leaf

Thanks for helping! Leaf is a small project with a clear plan, and this page explains how to fit into it. If
anything here is unclear, open an issue and ask; that is a useful contribution too.

## Ways to help

- **Report a bug.** Use the [bug report form](https://github.com/milinpaul/project-leaf/issues/new/choose). The
  most useful reports list the exact steps, what you expected and what happened, plus your OS.
- **Suggest a feature.** Read the [product spec](docs/product/PRODUCT_SPEC.md) first: it says what each phase
  covers and what is deliberately left out. Ideas outside the current phase are welcome as issues, but code for
  them will usually wait.
- **Fix something.** Issues labelled
  [`good first issue`](https://github.com/milinpaul/project-leaf/labels/good%20first%20issue) are small and well
  described. Stories with `status: todo` in [`backlog/stories/`](backlog/stories/) are planned work you can
  take on.
- **Improve the docs.** The docs are written for readers who do not know Rust, so plain-English fixes are
  very welcome.

**Before starting anything bigger than a small fix, comment on the issue (or open one)** so we can agree on the
approach and nobody else is working on the same thing.

## Development setup

Follow [Install](README.md#install) in the README first, then install the dev tools once:

```bash
brew install lefthook
```

(on Windows: `scoop install lefthook`), then

```bash
just setup
```

`just setup` installs `cargo-nextest`, `cargo-llvm-cov`, `cargo-deny`, `cargo-machete` and `typos`, adds the
WebAssembly target, and sets up git hooks that check formatting and spelling on commit, and run clippy and the
tests on push.

### Everyday tasks

| Command | What it does |
|---|---|
| `just` | List every task |
| `just run` | Build and launch the app |
| `just test` | Run all tests |
| `just check` | Everything CI runs: format, clippy, tests, coverage, licences, WebAssembly. Run it before opening a PR |
| `just fmt` | Format the code |
| `just cov-html` | Open a coverage report in the browser |
| `just fixture <folder>` | Make a large test workspace (5,000 pages) to try performance |
| `just screenshot <workspace> <out.png>` | Draw a workspace to an image without opening a window (macOS) |

## How the code is organised

Read these before changing code:

1. [docs/architecture/ARCHITECTURE.md](docs/architecture/ARCHITECTURE.md): the crates, and how a keystroke
   becomes a saved change.
2. [docs/architecture/BLOCK_MODEL.md](docs/architecture/BLOCK_MODEL.md): the data model and the commands that
   change it.
3. [docs/architecture/TECH_STACK.md](docs/architecture/TECH_STACK.md): the libraries and the crate dependency
   rules.

### Architecture rules

These are checked in review and will block a PR:

1. Core crates (`leaf-core`, `leaf-model`, `leaf-editor`, `leaf-store`, `leaf-export`, `leaf-search`) never
   import GPUI or `leaf-ui`.
2. Only `Command`s change documents. UI code never writes to Loro.
3. Editing logic lives in `leaf-editor` as plain, testable Rust. `leaf-ui` draws view models and forwards
   events, nothing more.
4. `leaf-core`, `leaf-model`, `leaf-editor` and `leaf-export` must compile for `wasm32-unknown-unknown`.
5. Never drop unknown data from a document; preserve it.
6. No network access in Phase 0.

### Code conventions

- Rust edition 2024 with the workspace lints on (clippy pedantic; warnings are errors).
- No `unwrap()` or `expect()` outside tests: return errors with `thiserror` types. No `unsafe`.
- Public items get doc comments that explain *why*, not just *what*.
- Small modules (under about 400 lines), one concept per file.
- A new dependency needs a reason in the PR description, and `just deny` must pass.

### Tests

- Every bug fix starts with a failing test.
- Every command and every editor rule in the spec's "Editor behaviour" section has a test.
- `proptest` for command sequences, undo round-trips and two-peer convergence; `insta` for view-model
  snapshots (review snapshot diffs, never accept them blindly).
- Core crates stay at 90% line coverage or above (`just cov`).

## The backlog

Planned work lives in [`backlog/`](backlog/README.md) as one markdown file per story, each with acceptance
criteria. If you pick up a story, say so on its issue or in a draft PR, and name the story ID (for example
`E07-S03`) in your PR title. You don't need to edit the story file; the maintainers keep the backlog up to date.

A bug report or idea that turns into planned work becomes a new story there.

## Pull requests

1. Fork the repository and make a branch from `main`.
2. Keep the PR focused on one change. Small PRs get reviewed faster.
3. Update the docs in the same PR when behaviour or structure changes: `ARCHITECTURE.md`, a crate's `//!`
   docs, or a new decision record in [docs/adr/](docs/adr/) (copy `0000-template.md` and add it to the index).
4. For visible UI changes, attach before and after screenshots.
5. Run `just check` and make sure it passes.
6. Fill in the PR template: what changed, why, how you tested it, and a short plain-English summary.

CI runs on every PR. A maintainer reviews and merges; we may ask for changes, which is a normal part of the
process.

## Licensing of contributions

Leaf is dual licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE). Unless you explicitly state
otherwise, any contribution you intentionally submit for inclusion in Leaf, as defined in the Apache-2.0
license, is dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
