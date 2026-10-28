//! GPUI components and theme that draw Leaf's view models and forward input events.
//!
//! This crate renders and nothing more: editing decisions belong in `leaf-editor`. It also wraps
//! every `gpui-component` widget the app uses, so a future change of UI framework touches only
//! this crate.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate, so keep logic out of it. Core
//! crates must never depend on it.

mod shell;

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::Root;

pub use shell::Shell;

/// Sets up themes and widgets. Call once at startup, before opening any window.
pub fn init(cx: &mut App) {
    gpui_component::init(cx);
}

/// Builds the root view of a Leaf window: the shell wrapped in gpui-component's `Root`, which
/// hosts dialogs, popovers and notifications. The app opens windows with this so it never
/// touches gpui-component itself.
pub fn window_root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    let shell = cx.new(|cx| Shell::new(window, cx));
    cx.new(|cx| Root::new(shell, window, cx))
}
