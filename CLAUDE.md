# CLAUDE.md

Leaf is a native, local-first Notion alternative written in Rust (GPUI UI, Loro CRDT model, SQLite storage).
Read these before any work: `docs/product/PRODUCT_SPEC.md` (what and why), `docs/architecture/TECH_STACK.md` (stack and crate rules),
`docs/architecture/BLOCK_MODEL.md` (data model and commands). `docs/README.md` maps the docs folder; `docs/adr/` holds past decisions.

## Current phase

**Phase 0: page editor.** Do not build databases, links, sync or export unless the task says so.

## Backlog (always on)

All work is tied to a story in `backlog/`. Use the **backlog skill** for every backlog action: choosing work, claiming,
planning, status changes, PRs, merges and new stories. Do this housekeeping yourself without being asked; the owner never
runs backlog commands. Hooks regenerate `backlog.json` and `board.html` automatically (local files, not committed).

## Commands

```
just            # list tasks
just check      # fmt + lint + test (run before declaring work done)
just test       # nextest, whole workspace
just cov        # coverage with 90% gate on core crates
just run        # launch the app
just install    # macOS: build Leaf and put it in /Applications
```

## Architecture rules (non-negotiable)

1. Core crates (`leaf-core`, `leaf-model`, `leaf-editor`, `leaf-store`, `leaf-export`, `leaf-search`) never import GPUI or `leaf-ui`.
2. Only `Command`s change documents. Never write to Loro from UI code.
3. All editing logic lives in `leaf-editor` as plain, testable Rust. `leaf-ui` renders view models and forwards events, nothing more.
4. Wasm-ok crates (`leaf-core`, `leaf-model`, `leaf-editor`, `leaf-export`) must compile for `wasm32-unknown-unknown`.
5. Never drop unknown data from a document; preserve it.
6. No network access anywhere in Phase 0.

## Code conventions

- Edition 2024, workspace lints on. No `unwrap()`/`expect()` outside tests; return errors with `thiserror` types.
- No `unsafe`.
- Public items get doc comments explaining *why*, not just *what*.
- Prefer small modules (< 400 lines). One concept per file.
- New dependency → justify it in the PR description and run `just deny`.

## Testing rules

- Every bug fix starts with a failing test.
- Every command and every editor rule in `docs/product/PRODUCT_SPEC.md` "Editor behaviour" has a test.
- Use `proptest` for command sequences, undo round-trips and two-peer convergence.
- Use `insta` for view-model snapshots. Review snapshot diffs, never blindly accept.
- Core crates must stay at ≥ 90% line coverage.

## Documentation rules

The owner does not write Rust and relies on these docs to understand the system. Keep them current in the same PR as the code:
- `docs/architecture/ARCHITECTURE.md`: crates, data flow, how a keystroke becomes a saved change. Update when structure changes.
- `docs/adr/NNNN-title.md`: one short decision record per significant architecture or feature decision. Copy `docs/adr/0000-template.md` and add it to the index in `docs/adr/README.md`.
- Each crate has a top-level `//!` doc explaining its role and boundaries.

## Definition of done

- `just check` passes locally; PR CI is green (PRs skip the test job; core-crate tests run in the coverage job).
- After merge, the `main` CI run is green on all three OSes. Don't start full-matrix runs on PR branches (owner's
  decision, 2026-10-29): check the `main` run after merge instead, and fix anything red on a new story branch. If that
  run was cancelled because a later merge queued behind it, check the newest `main` run that includes the change.
- Coverage gate passes.
- Docs updated (`docs/architecture/ARCHITECTURE.md` / ADR / crate docs) if behaviour or structure changed.
- PR description: what changed, why, how it was tested, plain-English summary for a non-Rust reader.
- No Claude session links (`Claude-Session:` lines, `claude.ai/code/session_…` URLs) in commits, PR descriptions or
  comments: the repository is public (owner's decision, 2026-10-30). The `Co-Authored-By` line is fine.

## When unsure

Stop and ask rather than guess on product behaviour. For technical choices, pick the simplest option that keeps the architecture rules, and record it in an ADR.
