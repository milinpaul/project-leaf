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

mod app;
mod cover_picker;
pub mod covers;
mod icon_picker;
pub mod menus;
mod notice;
mod search;
pub mod settings;
mod settings_dialog;
mod shell;
pub mod theme;
mod trash_dialog;
mod welcome;

use gpui::{App, AppContext as _, Entity, Window};
use gpui_component::Root;

pub use app::{AppView, app_root, refresh_menus};
pub use notice::Notice;
pub use shell::{Shell, Storage};

// Lucide icons Leaf draws beyond the ones gpui-component's widgets need. Listing them here embeds just
// these few, not the whole 1,800-icon set.
gpui_kit_assets::icon_assets!(
    LeafIcons,
    [
        ArrowDown,
        X,
        TextAlignStart,
        Hash,
        CircleQuestionMark,
        CircleChevronDown,
        Calendar,
        ArrowUp,
        Bold,
        Check,
        ChevronRight,
        Code,
        Copy,
        Ellipsis,
        ExternalLink,
        FaceSlightlySmilingPlus,
        FileText,
        Flag,
        GripVertical,
        Heading1,
        Heading2,
        Heading3,
        Image,
        ImagePlus,
        Italic,
        Link,
        List,
        ListOrdered,
        Menu,
        Minus,
        MoveHorizontal,
        PanelLeft,
        PencilLine,
        Repeat,
        RotateCcw,
        Search,
        SquareCheck,
        SquareCode,
        SquarePen,
        Star,
        StarFill,
        StarOff,
        Strikethrough,
        Table,
        TextQuote,
        Trash,
        Type
    ]
);

/// The icons Leaf and gpui-component's widgets draw (a checkbox's tick, the sidebar's compose button).
/// Register it with `Application::with_assets` at startup, or those icons render as blank shapes.
#[must_use]
pub fn assets() -> impl gpui::AssetSource {
    Assets
}

/// Leaf's cover images and extra icons first, then gpui-component's.
struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(cover) = covers::load(path) {
            return Ok(Some(cover));
        }
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
    shell::bind_page_keys(cx);
    shell::bind_database_keys(cx);
}

/// Builds the root view of a Leaf window: the shell wrapped in gpui-component's `Root`, which
/// hosts dialogs, popovers and notifications. The app opens windows with this so it never
/// touches gpui-component itself.
pub fn window_root(window: &mut Window, cx: &mut App) -> Entity<Root> {
    let shell = cx.new(|cx| Shell::new(window, cx));
    cx.new(|cx| Root::new(shell, window, cx))
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
