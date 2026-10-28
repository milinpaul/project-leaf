//! The Leaf desktop application: windows, menus and wiring between storage, editor and UI.
//!
//! This is the only binary. It assembles the other crates and handles platform glue, and holds
//! no logic of its own that could live in a testable crate instead.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate. Nothing depends on it.

use gpui::{App, Bounds, QuitMode, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_platform::application()
        .with_assets(leaf_ui::assets())
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            leaf_ui::init(cx);
            let bounds = Bounds::centered(None, size(px(1000.), px(700.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            };
            if let Err(err) = cx.open_window(options, leaf_ui::window_root) {
                eprintln!("leaf: could not open a window: {err:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
