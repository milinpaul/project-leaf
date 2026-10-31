//! The Leaf desktop application: windows, menus and wiring between storage, editor and UI.
//!
//! This is the only binary. It assembles the other crates and handles platform glue, and holds
//! no logic of its own that could live in a testable crate instead.
//!
//! **Boundaries:** UI crate, excluded from the coverage gate. Nothing depends on it.

use std::path::PathBuf;
use std::time::Instant;

use gpui::{App, Bounds, QuitMode, WindowBounds, WindowOptions, point, px, size};
use leaf_store::{DisplayArea, settings};
use leaf_ui::menus::{self, Quit};

/// Leaf's identity for the operating system: the macOS bundle identifier, the Windows taskbar group and
/// the Linux `app_id`, which must match the `.desktop` file name. Reverse-DNS under the owner's GitHub,
/// which no one else can claim (ADR-018).
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

/// Where the window opens: where it was last time if that is still on a screen, else centred at a
/// comfortable size.
fn restored_bounds(cx: &App) -> WindowBounds {
    let displays: Vec<DisplayArea> = cx
        .displays()
        .iter()
        .map(|display| {
            let area = display.bounds();
            DisplayArea {
                x: area.origin.x.into(),
                y: area.origin.y.into(),
                width: area.size.width.into(),
                height: area.size.height.into(),
            }
        })
        .collect();
    match leaf_ui::settings::get(cx).window_frame(&displays) {
        Some(frame) => {
            let bounds = Bounds {
                origin: point(px(frame.x), px(frame.y)),
                size: size(px(frame.width), px(frame.height)),
            };
            if frame.maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }
        }
        None => WindowBounds::Windowed(Bounds::centered(None, size(px(1000.), px(700.)), cx)),
    }
}

/// Where new workspaces go unless the owner picks another folder: a visible folder they can find
/// and back up.
fn default_location() -> PathBuf {
    std::env::home_dir()
        .unwrap_or_default()
        .join("Documents")
        .join("Leaf")
}

/// Set (to anything) to print how long the first frame took after launch: the cold-start budget
/// in `PRODUCT_SPEC.md` (under 500 ms to editable), checked in the Phase 0 acceptance pass.
const STARTUP_TIMING: &str = "LEAF_STARTUP_TIMING";

/// Prints the time from launch to the end of the window's first frame, if [`STARTUP_TIMING`] is set.
/// The first callback runs as that frame starts; asking for another frame makes the second run once
/// it has been drawn, so the number is a little over the true time, never under.
fn report_first_frame(started: Instant, window: &mut gpui::Window) {
    if std::env::var_os(STARTUP_TIMING).is_none() {
        return;
    }
    window.on_next_frame(move |window, _| {
        window.refresh();
        window.on_next_frame(move |_, _| {
            eprintln!(
                "leaf: first frame drawn {} ms after launch",
                started.elapsed().as_millis()
            );
        });
    });
}

fn main() {
    let started = Instant::now();
    // `leaf <path>` opens the workspace at <path>; plain `leaf` reopens the last one. With neither (the
    // first launch) or if it can't be opened, the welcome screen shows.
    let named = std::env::args_os().nth(1).map(PathBuf::from);
    gpui_platform::application()
        .with_assets(leaf_ui::assets())
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx: &mut App| {
            cx.set_app_identity(APP_ID, "Leaf");
            leaf_ui::init(settings::default_settings_path(), cx);
            // Quitting saves every unsaved change first (the shell and the settings flush on quit).
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys(menus::key_bindings());
            leaf_ui::refresh_menus(cx);
            let options = WindowOptions {
                window_bounds: Some(restored_bounds(cx)),
                app_id: Some(APP_ID.into()),
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                icon: window_icon(),
                ..Default::default()
            };
            if let Err(err) = cx.open_window(options, |window, cx| {
                report_first_frame(started, window);
                leaf_ui::app_root(named.clone(), default_location(), window, cx)
            }) {
                eprintln!("leaf: could not open a window: {err:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
