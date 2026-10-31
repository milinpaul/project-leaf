//! Fetching web bookmark previews in a (test) window (E15-S05), against a local server: no test reaches
//! the real internet.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{TestAppContext, VisualTestContext};
use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret};

use crate::shell::Shell;

const PAGE: &[u8] = b"<html><head><meta property=\"og:title\" content=\"The Leaf blog\">\
<meta name=\"description\" content=\"Notes from building Leaf.\"></head><body></body></html>";

/// A server answering every request with `PAGE` (or a 404 for `/missing`), counting page requests.
fn serve() -> (SocketAddr, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&count);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            // Pages only: a fetch also asks for `/favicon.ico` when the page names no icon.
            if !line.contains("/favicon.ico") {
                counted.fetch_add(1, Ordering::SeqCst);
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).unwrap_or(0) <= 2 {
                    break;
                }
            }
            let response = if line.contains("/missing") {
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
            } else {
                let mut out = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    PAGE.len()
                )
                .into_bytes();
                out.extend_from_slice(PAGE);
                out
            };
            let _ = stream.write_all(&response);
        }
    });
    (addr, count)
}

/// A page with an empty line, a fetcher that may reach the local server, and previews `allowed`.
fn open(
    cx: &mut TestAppContext,
    allowed: bool,
) -> (
    gpui::Entity<Shell>,
    &mut VisualTestContext,
    BlockId,
    String,
    Arc<AtomicUsize>,
) {
    let (addr, count) = serve();
    cx.update(|cx| {
        crate::init(None, cx);
        crate::settings::update(cx, |settings| settings.set_web_previews(allowed));
    });
    let (shell, cx) = cx.add_window_view(Shell::new);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let block = shell.update(cx, |shell, _| {
        shell.blocks.fetcher = Some(Arc::new(leaf_fetch::Fetcher::allowing(addr)));
        let page = shell.editor.open_page().unwrap().unwrap();
        let block = page.ensure_block().unwrap();
        page.set_caret(Caret { block, at: 0 }).unwrap();
        block
    });
    (shell, cx, block, format!("http://{addr}"), count)
}

/// Makes a bookmark of `url` in `block` the way the address box does, then lets fetches finish.
fn bookmark(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, url: &str) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.page().unwrap().insert_web_bookmark(url).unwrap();
            shell.changed(window, cx);
        });
    });
}

fn kind(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, block: BlockId) -> BlockKind {
    shell.update(cx, |shell, _| shell.page().unwrap().doc().kind(block).unwrap())
}

#[gpui::test]
fn a_new_bookmark_gets_its_preview_in_the_background(cx: &mut TestAppContext) {
    let (shell, cx, block, base, _) = open(cx, true);
    bookmark(&shell, cx, &format!("{base}/blog"));
    let fetching = shell.read_with(cx, |shell, _| shell.blocks.fetching.contains(&block));
    assert!(fetching, "the card says it is fetching");
    cx.run_until_parked();
    let BlockKind::WebBookmark { preview, .. } = kind(&shell, cx, block) else {
        panic!("not a bookmark");
    };
    assert_eq!(preview.title.as_deref(), Some("The Leaf blog"));
    assert_eq!(preview.description.as_deref(), Some("Notes from building Leaf."));
    assert!(preview.fetched_at.is_some());
    assert!(
        shell.read_with(cx, |shell, _| shell.blocks.fetching.is_empty()),
        "done"
    );
}

#[gpui::test]
fn nothing_goes_online_while_previews_are_not_allowed(cx: &mut TestAppContext) {
    let (shell, cx, block, base, count) = open(cx, false);
    bookmark(&shell, cx, &format!("{base}/blog"));
    cx.run_until_parked();
    assert_eq!(count.load(Ordering::SeqCst), 0, "no request at all");
    let BlockKind::WebBookmark { preview, .. } = kind(&shell, cx, block) else {
        panic!("not a bookmark");
    };
    assert!(preview.is_empty(), "the card shows the address");
}

#[gpui::test]
fn a_failed_fetch_leaves_the_bookmark_as_it_was(cx: &mut TestAppContext) {
    let (shell, cx, block, base, count) = open(cx, true);
    bookmark(&shell, cx, &format!("{base}/missing"));
    cx.run_until_parked();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let BlockKind::WebBookmark { preview, .. } = kind(&shell, cx, block) else {
        panic!("not a bookmark");
    };
    assert!(preview.is_empty());
    assert!(
        shell.read_with(cx, |shell, _| shell.blocks.fetching.is_empty()),
        "the loading line goes"
    );
}

#[gpui::test]
fn refresh_preview_in_the_block_menu_fetches_again(cx: &mut TestAppContext) {
    let (shell, cx, block, base, count) = open(cx, true);
    bookmark(&shell, cx, &format!("{base}/blog"));
    cx.run_until_parked();
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            let page = shell.page().unwrap();
            page.open_block_menu(block).unwrap();
            page.choose(leaf_editor::MenuEntry::RefreshPreview).unwrap();
            shell.changed(window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(count.load(Ordering::SeqCst), 2, "fetched twice");
}
