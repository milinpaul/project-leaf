# Tech Stack

## Summary

| Area | Choice | Notes |
|---|---|---|
| Language | Rust 1.99.1, edition 2024 | Pinned in `rust-toolchain.toml`; CI and dev machines use the same version. Bump deliberately in its own PR. |
| UI | GPUI + `gpui-component` (Longbridge) | Use the gpui version gpui-component pins. All GPUI usage stays inside `leaf-ui` and `leaf-app`. |
| Document model | `loro` 1.x | `LoroTree` for page and block hierarchy, `LoroText` for rich text, `UndoManager` for undo. |
| Persistence | SQLite via `rusqlite` (`bundled`) | Stores Loro snapshots + incremental updates, plus index tables. |
| Search (Phase 1) | `tantivy` | Full-text index, rebuilt from documents if lost. |
| Markdown (Phase 1) | `comrak` | Import/export only. |
| IDs | `uuid` v7 for pages and assets; Loro `TreeID` for blocks | See BLOCK_MODEL.md. |
| Hashing | `blake3` | Content-addressed image assets. |
| Errors | `thiserror` (libraries), `anyhow` (app binary only) | |
| Logging | `tracing`, `tracing-subscriber` | |
| Serialization | `serde`, `serde_json` | |
| Later: server | `axum`, `tokio`, Postgres, S3 | Cloud sync and team workspaces. |

## Crate layout

```
crates/
  leaf-core     ids, error types, shared primitives                 [core, wasm-ok]
  leaf-model    Loro docs, block schema, commands, undo, invariants [core, wasm-ok]
  leaf-editor   editor state: cursor, selection, key handling,
                slash menu, clipboard → commands; view models       [core, wasm-ok]
  leaf-store    SQLite persistence, autosave, crash recovery        [core, native]
  leaf-export   markdown import/export (Phase 1)                    [core, wasm-ok]
  leaf-search   tantivy index (Phase 1)                             [core, native]
  leaf-ui       GPUI components, theme, rendering of view models    [ui]
  leaf-app      binary: windows, menus, wiring, platform glue       [ui]
```

### Dependency rules (enforced in review)

1. **Core crates never depend on GPUI**, the file system APIs of the OS UI, or `leaf-ui`.
2. `wasm-ok` crates must compile for `wasm32-unknown-unknown` (CI checks this). No `std::fs`, threads or SQLite there.
3. `leaf-editor` contains **all** editing logic. `leaf-ui` only renders view models and forwards input events.
4. Dependency direction: `core ← model ← editor ← ui ← app`, with `store`, `export`, `search` depending only on `core`/`model`.
5. Every new third-party dependency needs a one-line justification in the PR and must pass `cargo-deny`.

## Workspace `Cargo.toml` (starter)

```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
leaf-core = { path = "crates/leaf-core" }
leaf-model = { path = "crates/leaf-model" }
leaf-editor = { path = "crates/leaf-editor" }
leaf-store = { path = "crates/leaf-store" }
leaf-ui = { path = "crates/leaf-ui" }
loro = "1"
rusqlite = { version = "0.37", features = ["bundled"] }
uuid = { version = "1", features = ["v7", "serde"] }
blake3 = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
tracing = "0.1"
# dev
proptest = "1"
insta = "1"
pretty_assertions = "1"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "warn"
expect_used = "warn"
todo = "warn"
dbg_macro = "warn"
module_name_repetitions = "allow"
missing_errors_doc = "allow"

[profile.dev.package."*"]
opt-level = 2

[profile.release]
lto = "thin"
codegen-units = 1
```

Each crate adds `[lints] workspace = true`. Versions above are starting points: resolve to the latest compatible releases at setup and commit `Cargo.lock`.

## Quality tooling

| Tool | Purpose |
|---|---|
| `cargo-nextest` | Test runner. |
| `insta` | Snapshot tests (view models, export output). |
| `proptest` | Property tests: random command sequences, merge convergence, undo round-trips. |
| GPUI `TestAppContext` | The few UI-level tests. |
| `cargo-llvm-cov` | Coverage, **≥ 90% lines on core crates** (CI gate). |
| `clippy` (pedantic) | Lints; warnings fail CI. |
| `rustfmt` | Formatting. |
| `typos` | Spell check. |
| `cargo-deny` | Licenses, advisories, duplicate/banned crates. |
| `cargo-machete` | Unused dependencies. |
| `divan` | Benchmarks for the performance budgets. |
| `just` | Task runner. |
| `lefthook` | Git hooks. |
| `cargo-dist` | Release installers (later). |

## Coverage policy

- Gate: **90% line coverage** across core crates (`leaf-core`, `leaf-model`, `leaf-editor`, `leaf-store`, later `leaf-export`, `leaf-search`).
- `leaf-ui` and `leaf-app` are excluded from the gate. Keep them thin: if logic can be tested, it belongs in `leaf-editor`.

## CI (GitHub Actions)

`fmt` → `clippy` → `typos` → `cargo-deny` + `cargo-machete` → `nextest` on macOS/Windows/Linux → WASM check of wasm-ok crates → coverage gate. See `.github/workflows/ci.yml`.

## GPUI notes

- The crates.io `gpui` lags behind Zed's main branch; `gpui-component` is the most active ecosystem. Pin exact versions and upgrade deliberately.
- Linux builds need system packages (X11/Wayland, xkbcommon, fontconfig, Vulkan); CI installs them.
- Wrap gpui-component widgets in `leaf-ui` so the app never imports them directly. This keeps a future framework swap contained.
