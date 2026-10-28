# Leaf documentation

| Folder | What lives there |
|---|---|
| [product/](product/) | What we are building and why. [PRODUCT_SPEC.md](product/PRODUCT_SPEC.md): vision, phases, editor rules, performance budgets. |
| [architecture/](architecture/) | How it is built. [TECH_STACK.md](architecture/TECH_STACK.md): libraries, crate layout and dependency rules. [BLOCK_MODEL.md](architecture/BLOCK_MODEL.md): data model, commands, storage. [ARCHITECTURE.md](architecture/ARCHITECTURE.md): the crates and how a keystroke becomes a saved change, in plain English. |
| [adr/](adr/) | Decision records: one file per significant architecture or feature decision. |

The code lives in [`crates/`](../crates/): one folder per crate, each starting with a `//!` comment in
`src/lib.rs` (or `src/main.rs` for `leaf-app`) that explains its role and boundaries in plain English.

The work itself is tracked in [`backlog/`](../backlog/README.md), not here.
