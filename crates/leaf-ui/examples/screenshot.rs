//! Renders a Leaf workspace in an off-screen window and saves it as a PNG, without showing anything
//! on screen: it works with the screen locked or the display asleep, and in CI. macOS only for now
//! (it uses the real Metal renderer through GPUI's visual test context).
//!
//! ```text
//! cargo run -p leaf-ui --example screenshot -- <workspace folder> <out.png> [options]
//! ```
//!
//! Opens the workspace's first page. Options:
//!
//! - `N` (a number): scroll so block N (counted from 0) is at the top.
//! - `dark`: the dark appearance. `nosidebar`: the sidebar hidden, as `Cmd/Ctrl+\` leaves it.
//! - `deep`: open the deepest page under the first one (its first child, and so on), for a breadcrumb.
//!   `star`: add the open page to Favourites. `cover=NAME`: give it the built-in cover NAME (`sage`,
//!   `lagoon`, …).
//! - `caret=B:C`: the caret at character C of block B; `select=B:C-B:C`: select from one to the
//!   other. Either gives the page the keyboard.
//! - `language=B`: open code block B's language picker.
//! - `link`: then open the link popover over the selection (Cmd/Ctrl+K); `type=TEXT` types into it.
//! - `type=TEXT`: then type TEXT through the platform text input. Avoid spaces: in this off-screen
//!   context, keys after a space don't arrive (the real app types spaces normally).
//! - `draft=TEXT`: then show TEXT as an input method's draft (composing, not yet written).
//! - `menu=B`: open block B's ⋮⋮ menu; `menu=B:kinds` also opens its "Turn into" list.
//! - `drag=B:T[:L]`: drag block B by its handle onto the lower half of block T, L indents in (default
//!   0), and keep holding: shows the drop line and the preview.
//! - `bench`: time frames while scrolling and while typing, and print the results.

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
    use std::time::{Duration, Instant};

    use gpui::{
        AnyWindowHandle, AppContext as _, Entity, EntityInputHandler as _, Modifiers, MouseButton,
        VisualTestAppContext, point, px, size,
    };
    use gpui_component::Root;
    use leaf_model::Caret;
    use leaf_store::Appearance;
    use leaf_ui::{Shell, Storage};

    type Error = Box<dyn std::error::Error>;

    /// What the command line asked for.
    #[allow(clippy::struct_excessive_bools, reason = "independent command-line switches")]
    struct Options {
        folder: String,
        out: String,
        scroll: Option<usize>,
        dark: bool,
        nosidebar: bool,
        deep: bool,
        star: bool,
        cover: Option<String>,
        bench: bool,
        caret: Option<String>,
        select: Option<String>,
        typed: Option<String>,
        draft: Option<String>,
        menu: Option<String>,
        drag: Option<String>,
        link: bool,
        language: Option<String>,
    }

    fn options() -> Result<Options, Error> {
        let usage = "usage: screenshot <workspace folder> <out.png> [N] [dark] [nosidebar] [deep] [star] [cover=NAME] [caret=B:C] [select=B:C-B:C] [link] [language=B] \
                     [type=TEXT] [draft=TEXT] [menu=B[:kinds]] [drag=B:T[:L]] [bench]";
        let mut args = std::env::args().skip(1);
        let folder = args.next().ok_or(usage)?;
        let out = args.next().ok_or(usage)?;
        let rest: Vec<String> = args.collect();
        let value = |prefix: &str| {
            rest.iter()
                .find_map(|a| a.strip_prefix(prefix).map(str::to_owned))
        };
        Ok(Options {
            folder,
            out,
            scroll: rest.iter().find_map(|a| a.parse().ok()),
            dark: rest.iter().any(|a| a == "dark"),
            nosidebar: rest.iter().any(|a| a == "nosidebar"),
            deep: rest.iter().any(|a| a == "deep"),
            star: rest.iter().any(|a| a == "star"),
            cover: value("cover="),
            bench: rest.iter().any(|a| a == "bench"),
            link: rest.iter().any(|a| a == "link"),
            language: value("language="),
            caret: value("caret="),
            select: value("select="),
            typed: value("type="),
            draft: value("draft="),
            menu: value("menu="),
            drag: value("drag="),
        })
    }

    pub fn run() -> Result<(), Error> {
        let options = options()?;
        let platform = gpui_platform::current_platform(false);
        let mut cx = VisualTestAppContext::with_asset_source(platform, Arc::new(leaf_ui::assets()));
        cx.update(|cx| leaf_ui::init(None, cx));

        let (storage, workspace) = Storage::open(leaf_store::Workspace::open(Path::new(&options.folder))?)?;
        let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
        let keep = made.clone();
        let window = cx.open_offscreen_window(size(px(1280.0), px(800.0)), move |window, cx| {
            let shell = cx.new(|cx| Shell::with_storage(storage, workspace, window, cx));
            *keep.borrow_mut() = Some(shell.clone());
            cx.new(|cx| Root::new(shell, window, cx))
        })?;
        let shell = made.borrow().clone().ok_or("the window has no shell")?;
        let window: AnyWindowHandle = window.into();

        prepare(&mut cx, window, &shell, &options)?;
        draw(&mut cx, window)?;
        if options.bench {
            bench(&mut cx, window, &shell)?;
        }
        let image = cx.capture_screenshot(window)?;
        image.save(&options.out)?;
        println!(
            "screenshot: saved {} ({}x{})",
            options.out,
            image.width(),
            image.height()
        );
        Ok(())
    }

    /// The first page in the sidebar, or with `deep` its first child's first child and so on.
    fn first_page(shell: &Shell, deep: bool) -> Option<leaf_core::PageId> {
        let workspace = shell.editor().workspace();
        let mut page = shell.editor().sidebar().ok()?.first()?.page;
        if deep {
            while let Some(child) = workspace.children(page).ok().and_then(|c| c.first().copied()) {
                page = child;
            }
        }
        Some(page)
    }

    /// Opens the first page and applies the options: scroll, appearance, caret, selection, typing.
    fn prepare(
        cx: &mut VisualTestAppContext,
        window: AnyWindowHandle,
        shell: &Entity<Shell>,
        options: &Options,
    ) -> Result<(), Error> {
        cx.update_window(window, |_, window, cx| {
            if options.dark {
                leaf_ui::theme::set_appearance(Appearance::Dark, window, cx);
            }
            shell.update(cx, |shell, cx| {
                if options.nosidebar {
                    shell.toggle_sidebar(false, window, cx);
                }
                if let Some(page) = first_page(shell, options.deep) {
                    shell.open(page, window, cx);
                    if options.star {
                        let _ = shell.editor_mut().set_favourite(page, true);
                    }
                    let cover = options.cover.as_deref().and_then(|name| {
                        leaf_editor::BuiltinCover::ALL
                            .iter()
                            .copied()
                            .find(|c| c.name() == name)
                    });
                    if let Some(cover) = cover {
                        let _ = shell.editor_mut().set_cover(page, Some(cover));
                    }
                }
                if let Some(block) = options.scroll {
                    shell.scroll_page_to(block, cx);
                }
                if let Some(page) = shell.editor_mut().open_page().ok().flatten() {
                    let rows = page.rows().unwrap_or_default();
                    let caret = |spec: &str| -> Option<Caret> {
                        let (block, at) = spec.split_once(':')?;
                        Some(Caret {
                            block: rows.get(block.parse::<usize>().ok()?)?.id,
                            at: at.parse().ok()?,
                        })
                    };
                    if let Some(at) = options.caret.as_deref().and_then(caret) {
                        let _ = page.set_caret(at);
                    }
                    if let Some((from, to)) = options.select.as_deref().and_then(|s| s.split_once('-'))
                        && let (Some(from), Some(to)) = (caret(from), caret(to))
                    {
                        let _ = page.select_between(from, to);
                    }
                }
                if options.caret.is_some() || options.select.is_some() {
                    shell.focus_page(window, cx);
                }
            });
        })?;
        if options.link {
            // Placed by where the selection was drawn, so draw it first.
            draw(cx, window)?;
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| shell.open_link_popover(window, cx));
            })?;
        }
        if let Some(text) = &options.typed {
            // One character at a time with a frame in between, as a person types.
            for ch in text.chars() {
                draw(cx, window)?;
                type_char(cx, window, ch);
            }
        }
        if let Some(draft) = &options.draft {
            draw(cx, window)?;
            cx.update_window(window, |_, window, cx| {
                let len = draft.encode_utf16().count();
                shell.update(cx, |shell, cx| {
                    shell.replace_and_mark_text_in_range(None, draft, Some(len..len), window, cx);
                });
            })?;
        }
        if let Some(spec) = &options.menu {
            draw(cx, window)?;
            let (block, kinds) = spec
                .split_once(':')
                .map_or((spec.as_str(), false), |(b, k)| (b, k == "kinds"));
            let index: usize = block.parse()?;
            cx.update_window(window, |_, window, cx| {
                shell.update(cx, |shell, cx| {
                    let id = shell
                        .editor_mut()
                        .open_page()
                        .ok()
                        .flatten()
                        .and_then(|page| page.rows().ok())
                        .and_then(|rows| rows.get(index).map(|row| row.id));
                    if let Some(id) = id {
                        shell.open_block_menu_beside(id, window, cx);
                        if kinds && let Some(page) = shell.editor_mut().open_page().ok().flatten() {
                            let _ = page.block_menu_key(leaf_editor::MenuKey::Right);
                        }
                    }
                });
            })?;
        }
        if let Some(block) = &options.language {
            language(cx, window, shell, block)?;
        }
        if let Some(spec) = &options.drag {
            drag(cx, window, shell, spec)?;
        }
        Ok(())
    }

    /// Opens code block B's language picker.
    fn language(
        cx: &mut VisualTestAppContext,
        window: AnyWindowHandle,
        shell: &Entity<Shell>,
        block: &str,
    ) -> Result<(), Error> {
        draw(cx, window)?;
        let index: usize = block.parse()?;
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                let rows = shell
                    .editor_mut()
                    .open_page()
                    .ok()
                    .flatten()
                    .and_then(|p| p.rows().ok());
                if let Some(id) = rows.and_then(|rows| rows.get(index).map(|r| r.id)) {
                    shell.open_language_picker_beside(id, window, cx);
                }
            });
        })?;
        Ok(())
    }

    /// Presses on block B's handle and moves onto block T (`B:T[:L]`), without letting go.
    fn drag(
        cx: &mut VisualTestAppContext,
        window: AnyWindowHandle,
        shell: &Entity<Shell>,
        spec: &str,
    ) -> Result<(), Error> {
        draw(cx, window)?;
        let mut parts = spec.split(':');
        let from: usize = parts.next().ok_or("drag=B:T")?.parse()?;
        let to: usize = parts.next().ok_or("drag=B:T")?.parse()?;
        let levels: f32 = parts.next().map_or(Ok(0.0), str::parse)?;
        let points = cx.update_window(window, |_, _, cx| {
            shell.update(cx, |shell, _| {
                let rows = shell.editor_mut().open_page().ok().flatten()?.rows().ok()?;
                let (from, to) = (rows.get(from)?.id, rows.get(to)?.id);
                shell.block_drag_points(from, to, levels)
            })
        })?;
        let (start, end) = points.ok_or("no such rows on screen")?;
        cx.simulate_mouse_down(window, start, MouseButton::Left, Modifiers::none());
        for step in 1..=8_u8 {
            let t = f32::from(step) / 8.0;
            let at = point(start.x + (end.x - start.x) * t, start.y + (end.y - start.y) * t);
            cx.simulate_mouse_move(window, at, MouseButton::Left, Modifiers::none());
            draw(cx, window)?;
        }
        Ok(())
    }

    /// Draws explicitly (off screen, nothing asks for frames). Two frames: the list measures rows on
    /// the first, then lays out with their real heights.
    fn draw(cx: &mut VisualTestAppContext, window: AnyWindowHandle) -> Result<(), Error> {
        for _ in 0..2 {
            cx.run_until_parked();
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
        }
        Ok(())
    }

    /// Times frames: scrolling a line at a time, jumping across the page, and typing.
    fn bench(
        cx: &mut VisualTestAppContext,
        window: AnyWindowHandle,
        shell: &Entity<Shell>,
    ) -> Result<(), Error> {
        let blocks = shell.read_with(cx, |shell, _| shell.page_block_count());
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
                cx.update_window(window, |_, window, cx| {
                    shell.update(cx, |shell, cx| shell.scroll_page_to(block, cx));
                    let started = Instant::now();
                    window.draw(cx).clear(cx);
                    frames.push(started.elapsed());
                })?;
            }
            report(blocks, name, frames);
        }
        // Typing in the middle of the page: each keystroke goes through the platform text input, then a
        // frame is drawn.
        cx.update_window(window, |_, window, cx| {
            shell.update(cx, |shell, cx| {
                shell.scroll_page_to(blocks / 2, cx);
                let middle = shell
                    .editor_mut()
                    .open_page()
                    .ok()
                    .flatten()
                    .and_then(|page| page.rows().ok())
                    .and_then(|rows| rows.get(blocks / 2).map(|row| row.id));
                if let Some(block) = middle {
                    shell.focus_page_at(Caret { block, at: 0 }, window, cx);
                }
            });
        })?;
        draw(cx, window)?;
        let mut frames = Vec::new();
        for ch in "the quick brown fox jumps over the lazy dog "
            .chars()
            .cycle()
            .take(200)
        {
            let started = Instant::now();
            type_char(cx, window, ch);
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
            frames.push(started.elapsed());
        }
        let slow: Vec<(usize, u128)> = frames
            .iter()
            .enumerate()
            .filter(|(_, d)| d.as_millis() >= 12)
            .map(|(i, d)| (i, d.as_millis()))
            .collect();
        println!("bench: keystrokes over 12 ms (index, ms): {slow:?}");
        report(blocks, "typing, keystroke to painted frame", frames);
        Ok(())
    }

    /// Presses the key for `ch` as a keyboard does. (The visual test context's own `simulate_input`
    /// names the space key " " rather than "space", which no keyboard sends.)
    fn type_char(cx: &mut VisualTestAppContext, window: AnyWindowHandle, ch: char) {
        let key = if ch == ' ' {
            "space".to_owned()
        } else {
            ch.to_string()
        };
        cx.dispatch_keystroke(
            window,
            gpui::Keystroke {
                modifiers: gpui::Modifiers::default(),
                key,
                key_char: Some(ch.to_string()),
            },
        );
        cx.run_until_parked();
    }

    fn report(blocks: usize, name: &str, mut frames: Vec<Duration>) {
        frames.sort();
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        println!(
            "bench: {blocks} blocks, {name}: median {:.2} ms, 95th percentile {:.2} ms, slowest {:.2} ms",
            ms(frames[frames.len() / 2]),
            ms(frames[frames.len() * 95 / 100]),
            ms(frames[frames.len() - 1]),
        );
    }
}
