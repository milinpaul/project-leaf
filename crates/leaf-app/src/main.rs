//! The Leaf desktop application: windows, menus and wiring between storage, editor and UI.
//!
//! This is the only binary. It assembles the other crates and handles platform glue, and holds
//! no logic of its own that could live in a testable crate instead.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate. Nothing depends on it.

use std::path::PathBuf;

use gpui::{
    App, Bounds, KeyBinding, Menu, MenuItem, QuitMode, WindowBounds, WindowOptions, actions, px, size,
};

actions!(
    leaf,
    [
        /// Quits Leaf. Quitting saves every unsaved change first (the shell flushes on quit).
        Quit
    ]
);

/// Leaf's identity for the operating system: the macOS bundle identifier, the Windows taskbar group and
/// the Linux `app_id`, which must match the `.desktop` file name. Reverse-DNS under the owner's GitHub,
/// which no one else can claim (ADR 0018).
const APP_ID: &str = "io.github.milinpaul.Leaf";

/// The window icon for X11, which reads it from the window. Wayland, macOS and Windows take the icon from
/// the `.desktop` entry, the app bundle and the executable instead.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    let png = include_bytes!("../resources/png/leaf-256.png");
    match image::load_from_memory_with_format(png, image::ImageFormat::Png) {
        Ok(icon) => Some(std::sync::Arc::new(icon.into_rgba8())),
        Err(err) => {
            eprintln!("leaf: could not load the window icon: {err}");
            None
        }
    }
}

/// Where the workspace lives when no path is given: a visible folder the user can find and back up.
fn default_workspace() -> PathBuf {
    std::env::home_dir()
        .unwrap_or_default()
        .join("Documents")
        .join("Leaf")
        .join("My Workspace.leaf")
}

fn main() {
    // `leaf <path>` opens (or creates) the workspace at <path>; plain `leaf` uses the default.
    let path = std::env::args_os()
        .nth(1)
        .map_or_else(default_workspace, PathBuf::from);
    gpui_platform::application()
        .with_assets(leaf_ui::assets())
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            cx.set_app_identity(APP_ID, "Leaf");
            leaf_ui::init(cx);
            // Cmd+Q on macOS, Ctrl+Q elsewhere, plus the standard app menu entry.
            cx.on_action(|_: &Quit, cx| cx.quit());
            let quit = if cfg!(target_os = "macos") {
                "cmd-q"
            } else {
                "ctrl-q"
            };
            cx.bind_keys([KeyBinding::new(quit, Quit, None)]);
            cx.set_menus([Menu {
                name: "Leaf".into(),
                items: vec![MenuItem::action("Quit Leaf", Quit)],
                disabled: false,
            }]);
            let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                app_id: Some(APP_ID.into()),
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                icon: window_icon(),
                ..Default::default()
            };
            if let Err(err) = cx.open_window(options, |window, cx| leaf_ui::workspace_root(&path, window, cx))
            {
                eprintln!("leaf: could not open a window: {err:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
