//! GPUI components and theme that draw Leaf's view models and forward input events.
//!
//! This crate renders and nothing more: editing decisions belong in `leaf-editor`. It also wraps
//! every `gpui-component` widget the app uses, so a future change of UI framework touches only
//! this crate.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate, so keep logic out of it. Core
//! crates must never depend on it.
