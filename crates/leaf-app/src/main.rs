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
