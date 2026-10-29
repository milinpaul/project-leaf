//! Renders a Leaf workspace in an off-screen window and saves it as a PNG, without showing anything
//! on screen: it works with the screen locked or the display asleep, and in CI. macOS only for now
//! (it uses the real Metal renderer through GPUI's visual test context).
//!
//! ```text
//! cargo run -p leaf-ui --example screenshot -- <workspace folder> <out.png> [first block] [dark]
//! ```
//!
//! Opens the workspace's first page. With `first block`, scrolls so that block (counted from 0) is
//! at the top. With `dark`, uses the dark appearance. With `bench`, also scrolls through the
//! page, timing how long each frame takes to lay out and paint, and prints the results.

fn main() {
    #[cfg(target_os = "macos")]
    if let Err(err) = mac::run() {
        eprintln!("screenshot: {err}");
        std::process::exit(1);
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("screenshot: macOS only for now (GPUI's off-screen visual test context is macOS-only)");
        std::process::exit(1);
    }
}

/// GPUI provides `VisualTestAppContext` only on macOS.
#[cfg(target_os = "macos")]
mod mac {
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{AppContext as _, Entity, VisualTestAppContext, px, size};
    use gpui_component::Root;
    use leaf_store::Appearance;
    use leaf_ui::{Shell, Storage};

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let usage = "usage: screenshot <workspace folder> <out.png> [first block] [dark]";
        let mut args = std::env::args().skip(1);
        let folder = args.next().ok_or(usage)?;
        let out = args.next().ok_or(usage)?;
        let rest: Vec<String> = args.collect();
        let scroll: Option<usize> = rest.iter().find_map(|a| a.parse().ok());
        let dark = rest.iter().any(|a| a == "dark");
        let bench = rest.iter().any(|a| a == "bench");

        let platform = gpui_platform::current_platform(false);
        let mut cx = VisualTestAppContext::with_asset_source(platform, Arc::new(leaf_ui::assets()));
        cx.update(|cx| leaf_ui::init(None, cx));

        let (storage, workspace) = Storage::open(leaf_store::Workspace::open(Path::new(&folder))?)?;
        let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
        let keep = made.clone();
        let window = cx.open_offscreen_window(size(px(1280.0), px(800.0)), move |window, cx| {
            let shell = cx.new(|cx| Shell::with_storage(storage, workspace, window, cx));
            *keep.borrow_mut() = Some(shell.clone());
            cx.new(|cx| Root::new(shell, window, cx))
        })?;
        let shell = made.borrow().clone().ok_or("the window has no shell")?;

        cx.update_window(window.into(), |_, window, cx| {
            if dark {
                leaf_ui::theme::set_appearance(Appearance::Dark, window, cx);
            }
            shell.update(cx, |shell, cx| {
                let first = shell
                    .editor()
                    .sidebar()
                    .ok()
                    .and_then(|items| items.first().map(|i| i.page));
                if let Some(page) = first {
                    shell.open(page, window, cx);
                }
                if let Some(block) = scroll {
                    shell.scroll_page_to(block, cx);
                }
            });
        })?;
        // Draw explicitly (off screen nothing asks for frames). Two frames: the list measures rows on
        // the first, then lays out with their real heights.
        for _ in 0..2 {
            cx.run_until_parked();
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
        }
        if bench {
            let blocks = shell.read_with(&cx, |shell, _| shell.page_block_count());
            // Scrolling like a trackpad: one block (about one line) further each frame, from the middle.
            let steady: Vec<usize> = (0..300).map(|i| blocks / 2 + i).collect();
            // Jumping around the page, as when dragging the scrollbar or opening a search result.
            let jumps: Vec<usize> = (0..50).map(|i| blocks * i / 50).collect();
            for (name, positions) in [
                ("scrolling a line per frame", steady),
                ("jumping across the page", jumps),
            ] {
                let mut frames = Vec::new();
                for block in positions {
                    cx.update_window(window.into(), |_, window, cx| {
                        shell.update(cx, |shell, cx| shell.scroll_page_to(block, cx));
                        let started = std::time::Instant::now();
                        window.draw(cx).clear(cx);
                        frames.push(started.elapsed());
                    })?;
                }
                frames.sort();
                let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
                println!(
                    "bench: {blocks} blocks, {name}: median frame {:.2} ms, 95th percentile {:.2} ms, slowest {:.2} ms",
                    ms(frames[frames.len() / 2]),
                    ms(frames[frames.len() * 95 / 100]),
                    ms(frames[frames.len() - 1]),
                );
            }
        }
        let image = cx.capture_screenshot(window.into())?;
        image.save(&out)?;
        println!("screenshot: saved {out} ({}x{})", image.width(), image.height());
        Ok(())
    }
}
