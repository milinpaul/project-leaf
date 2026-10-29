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

mod icon_picker;
pub mod menus;
mod notice;
pub mod settings;
mod settings_dialog;
mod shell;
mod switcher;
pub mod theme;

use std::path::Path;

use gpui::{AnyView, App, AppContext as _, Entity, Window};
use gpui_component::Root;

pub use notice::Notice;
pub use shell::{Shell, Storage};

// Lucide icons Leaf draws beyond the ones gpui-component's widgets need. Listing them here embeds just
// these few, not the whole 1,800-icon set.
gpui_kit_assets::icon_assets!(
    LeafIcons,
    [FaceSlightlySmilingPlus, FileText, PencilLine, SquarePen, Trash]
);

/// The icons Leaf and gpui-component's widgets draw (a checkbox's tick, the sidebar's compose button).
/// Register it with `Application::with_assets` at startup, or those icons render as blank shapes.
#[must_use]
pub fn assets() -> impl gpui::AssetSource {
    Assets
}

/// Leaf's extra icons first, then gpui-component's.
struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match LeafIcons.load(path)? {
            Some(bytes) => Ok(Some(bytes)),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        let mut paths = gpui_kit_assets::Assets.list(path)?;
        paths.extend(LeafIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
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

#[cfg(test)]
mod tests {
    use gpui::AssetSource as _;
    use gpui_kit_assets::IconName;

    #[test]
    fn leaf_icons_and_widget_icons_both_load() {
        let assets = super::assets();
        for icon in [
            IconName::Trash,
            IconName::SquarePen,
            IconName::PencilLine,
            IconName::FileText,
            IconName::Plus,
        ] {
            let bytes = assets.load(icon.path().as_ref()).unwrap();
            assert!(bytes.is_some_and(|svg| svg.starts_with(b"<svg")), "{icon:?}");
        }
        let missing = assets.load("icons/no-such-icon.svg");
        assert!(
            missing.is_err() || missing.is_ok_and(|bytes| bytes.is_none()),
            "an unknown icon is not invented"
        );
        let listed = assets.list("icons/").unwrap();
        assert!(listed.iter().any(|path| path.as_ref() == "icons/trash.svg"));
        assert!(
            listed.windows(2).all(|pair| pair[0] < pair[1]),
            "sorted, no duplicates"
        );
    }
}
