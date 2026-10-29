//! GPUI components and theme that draw Leaf's view models and forward input events. Leaf's look
//! (colours, fonts, sizes) lives in [`theme`]; the app settings while Leaf runs in [`settings`]; menus
//! and shortcuts in [`menus`].
//!
//! This crate renders and nothing more: editing decisions belong in `leaf-editor`. It also wraps
//! every `gpui-component` widget the app uses, so a future change of UI framework touches only
//! this crate.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate, so keep logic out of it. Core
//! crates must never depend on it.

pub mod menus;
mod notice;
pub mod settings;
mod settings_dialog;
mod shell;
pub mod theme;

use std::path::Path;

use gpui::{AnyView, App, AppContext as _, Entity, Window};
use gpui_component::Root;

pub use notice::Notice;
pub use shell::{Shell, Storage};

/// The icons gpui-component's widgets draw (a checkbox's tick, for example). Register it with
/// `Application::with_assets` at startup, or those icons render as blank shapes.
#[must_use]
pub fn assets() -> impl gpui::AssetSource {
    gpui_kit_assets::Assets
}

/// Sets up widgets, loads the app settings from `settings_path` (defaults if `None`), then Leaf's fonts
/// and its light and dark themes. Call once at startup, before opening any window.
pub fn init(settings_path: Option<std::path::PathBuf>, cx: &mut App) {
    gpui_component::init(cx);
    settings::init(settings_path, cx);
    theme::init(cx);
}

/// Builds the root view of a Leaf window: the shell wrapped in gpui-component's `Root`, which
/// hosts dialogs, popovers and notifications. The app opens windows with this so it never
/// touches gpui-component itself.
pub fn window_root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    let shell = cx.new(|cx| Shell::new(window, cx));
    cx.new(|cx| Root::new(shell, window, cx))
}

/// Builds the root view of a window editing the workspace at `path`, creating the workspace if
/// nothing is there yet. If it can't be opened (another Leaf has it open, it is damaged, ...), the
/// window explains why instead.
pub fn workspace_root(path: &Path, window: &mut Window, cx: &mut App) -> Entity<Root> {
    let opened = leaf_store::Workspace::open_or_create(path).and_then(shell::Storage::open);
    let view: AnyView = match opened {
        Ok((storage, workspace)) => cx
            .new(|cx| Shell::with_storage(storage, workspace, window, cx))
            .into(),
        Err(err) => cx
            .new(|_| Notice::new("Leaf can't open this workspace", err.to_string()))
            .into(),
    };
    cx.new(|cx| Root::new(view, window, cx))
}
