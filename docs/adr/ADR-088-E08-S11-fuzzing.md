# ADR-088. Fuzzing the code that reads outside input, with cargo-fuzz in its own crate

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S11

## Context

`SECURITY.md` names the threat that matters most for a local-first app: a crafted or damaged workspace, image or
pasted text that crashes Leaf or makes it lose data. Proptest covers command sequences built from valid steps, but
nothing fed Leaf arbitrary bytes. Four places read input Leaf didn't make itself: saved document bytes (Loro
snapshots and updates), the workspace database file, dropped or pasted files (type sniffing and the SVG check),
and pasted text and clipboard fragments. Leaf has no HTML paste parser: the clipboard hands it plain text or its own
fragment JSON.

## Decision

- **cargo-fuzz (libFuzzer) in a `fuzz/` crate** with its own `[workspace]` and lock file. The main workspace
  excludes it, so `just check`, CI's PR jobs and normal builds never compile it, and the pinned stable toolchain stays
  as it is. cargo-fuzz needs nightly Rust; only `just fuzz` uses it.
- **Four targets:** `doc_bytes` (page, workspace and database documents loaded from a snapshot plus updates, through
  recovery, and as merged changes), `workspace_db` (the bytes as `workspace.sqlite` and `view.json`, opened the way
  the app opens a workspace, then every page and database loaded), `asset_kind` (`image_mime`, `check_svg`,
  `import_asset` and `drawable_asset`) and `paste` (`paste_text` and fragment `paste` into a small page, then undo).
- **What counts as a finding:** a panic, an input over 5 seconds (a hang) or over 2 GB of memory. Each target also
  asserts what must stay true: a document that loaded saves and loads again, and a page keeps its rules after a
  paste. Errors are the right answer to bad bytes and are ignored.
- **Seeds** come from `crates/leaf-fixtures/examples/fuzz_seeds.rs`: fixture pages with links (with and without a
  later update), a workspace tree with a database, a saved workspace folder, image headers, SVGs to accept and to
  refuse, Markdown of every block kind and clipboard fragments. The corpus is generated, not committed.
- **AddressSanitizer only for `workspace_db`:** it is the one target that runs C code (bundled SQLite). Leaf's Rust
  has no `unsafe`, so elsewhere the sanitizer only slows runs down (about ten times for `paste`).
- **A panic inside Loro is an error, not a crash.** The first ten-minute run of `doc_bytes` found a damaged
  update that makes Loro 1.17.1 (the newest release) panic on an assertion while it reads the update's header,
  instead of returning an error. A page with such an update in its saved rows would crash Leaf on open, before
  recovery could skip the update. Every import in `leaf-model` now goes through `loro_import`, which catches a
  panic there (`catch_unwind`) and returns `ModelError::Damaged`; recovery then skips the update like any other
  damaged one. The input is kept as a regression test (`crates/leaf-model/tests/damaged_bytes.rs`). The fuzz
  targets put Rust's normal panic hook back, since libFuzzer's aborts even on a caught panic.
- **CI:** `.github/workflows/fuzz.yml` runs each target for 5 minutes weekly and on demand, never on pull requests,
  and keeps any finding as a run artifact.

## Alternatives considered

- **Only more proptest:** proptest builds valid values; it rarely produces a damaged Loro update or SQLite page,
  which is the input the threat model is about.
- **cargo-fuzz targets inside the main workspace:** every build and `cargo deny` run would see libFuzzer and need
  nightly.
- **Waiting for a Loro fix instead of catching the panic:** the crash is on Leaf's open path today. The catch is
  small and stays useful for the next decoder panic; it can go once Loro reports all bad input as errors.
- **AFL++ or honggfuzz:** both work with Rust, but cargo-fuzz is the Rust project's own tool, needs nothing beyond
  nightly, and its targets are plain functions.

## Consequences

- `just fuzz <target> [seconds]` (default 60) runs a target locally; findings land in `fuzz/artifacts/`. A finding is
  fixed with a failing regression test first, or filed as an E17 bug story.
- The fuzz crate is outside `just check`, so a refactor can break it unnoticed until the next weekly run; that run
  fails loudly when it doesn't build.
- Catching a panic assumes Loro hasn't half-changed the document. Every load imports into a fresh document and
  drops it on error, so that holds there; merging into an open document (`import_changes`, used only by tests in
  Phase 0) should be looked at again before sync.
- New code that reads outside input (a future HTML paste, import of other apps' exports) should get a target here.
- Plain English: Leaf now has a tool that throws millions of random and broken files and pastes at it, looking for
  any that crash it. It runs once a week on GitHub and whenever a developer asks.
