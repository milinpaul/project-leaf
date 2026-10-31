# ADR-090. Lints against hidden panics: no panic macros, checked indexing and arithmetic in core crates

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-31
- **Story:** E08-S15

## Context

The workspace already forbade `unsafe` and warned on `unwrap`/`expect` and `clippy::pedantic`. Panics could still
hide in `panic!`/`unreachable!`, in slice indexing (`v[i]`, `s[a..b]`) and in integer arithmetic that overflows.
The core crates read saved data and pasted text that may be damaged or crafted (E08-S11 found Loro panicking on
such bytes), so a panic there can crash the app on input it should refuse. Several `#[allow]`s also said nothing
about why.

## Decision

- **Every crate** warns (CI treats warnings as errors) on `allow_attributes_without_reason`, `panic`,
  `unreachable`, `print_stdout`, `print_stderr`, and names `cast_possible_truncation` and `cast_sign_loss` (on
  already through `pedantic`).
- **Core crates** (`leaf-core`, `leaf-model`, `leaf-editor`, `leaf-store`, `leaf-export`, `leaf-search`) also warn
  on `indexing_slicing` and `arithmetic_side_effects`, set in each `lib.rs` with `cfg_attr(not(test), ...)` so unit
  tests stay readable. The UI crates draw and lay out with numbers everywhere and only forward events, so they are
  left out.
- **Fixes, in order:** `get`, iterators and pattern matching; `checked_*` where the function already returns
  `Option`/`Result`; `saturating_*` where clamping is the meaning; otherwise `#[expect(lint, reason = "...")]` on the
  smallest item, saying why it can't fail. A fix only changes what happens in a case that used to panic.
- **Tests** may panic, index and print (`clippy.toml`); integration-test helpers outside `#[test]` functions say so
  with a file-level `allow` and a reason. Examples and benches may print.
- **One place prints:** the app's error lines go through `leaf_ui::log::log` (`leaf: ...` on standard error), the
  only `print_stderr` in the app, so a log file or on-screen notice later changes one function.

## Alternatives considered

- **Only `panic`/`unreachable`:** misses the commonest hidden panics, indexing and overflow.
- **Indexing and arithmetic lints in every crate:** the UI is mostly pixel arithmetic on small values; hundreds of
  `expect`s there would bury the ones that matter.
- **`deny` instead of `warn`:** CI already fails on warnings; `warn` keeps local builds usable mid-edit.

## Consequences

- New core code reaches for `get` and checked arithmetic first; a raw index or `+` needs a reason that a reviewer
  can check.
- `#[expect]` (unlike `#[allow]`) warns when the lint no longer fires, so stale exceptions get cleaned up.
- Plain English: Leaf's rules now refuse code that could crash on an unexpected number or position, unless the
  author writes down why it can't happen.
