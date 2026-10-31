# Leaf documentation

| Folder | What lives there |
|---|---|
| [product/](product/) | What we are building and why. [PRODUCT_SPEC.md](product/PRODUCT_SPEC.md): vision, phases, editor rules, performance budgets. [PHASE0_ACCEPTANCE.md](product/PHASE0_ACCEPTANCE.md): the evidence for each Phase 0 acceptance item. |
| [architecture/](architecture/) | How it is built. [TECH_STACK.md](architecture/TECH_STACK.md): libraries, crate layout and dependency rules. [BLOCK_MODEL.md](architecture/BLOCK_MODEL.md): data model, commands, storage. [ARCHITECTURE.md](architecture/ARCHITECTURE.md): the crates and how a keystroke becomes a saved change, in plain English. |
| [screenshots/](screenshots/) | Screenshots of the running app attached to PRs with visible changes, named by story. |
| [spikes/](spikes/) | Reports from time-boxed experiments (e.g. [E14-S01 web editor](spikes/E14-S01-web-editor.md)): what was tried, measured and learned. The throwaway code lives in [`spikes/`](../spikes/) at the repository root, outside the workspace build. |
| [adr/](adr/) | Decision records: one file per significant architecture or feature decision, named `ADR-NNN-EXX-SYY-title.md` (order taken, then story; new ones are `ADR-XXX-…` until merged, then numbered automatically). |

The code lives in [`crates/`](../crates/): one folder per crate, each starting with a `//!` comment in
`src/lib.rs` (or `src/main.rs` for `leaf-app`) that explains its role and boundaries in plain English.

The work itself is tracked in [`backlog/`](../backlog/README.md), not here.
