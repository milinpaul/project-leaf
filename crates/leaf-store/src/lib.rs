//! Persistence for Leaf workspaces: SQLite storage of Loro documents, autosave and crash recovery.
//!
//! Documents are saved as Loro snapshots plus small incremental updates, so typing is durable
//! within a second without rewriting whole pages. Isolating storage here keeps the model and
//! editor portable (they also run in WASM) and lets us swap or extend storage later, for
//! example for cloud sync.
//!
//! **Boundaries:** core crate, native only. No GPUI and no `leaf-ui`. Depends only on
//! `leaf-core` and `leaf-model`.
