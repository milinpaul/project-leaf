//! The search bar in a real window: Cmd/Ctrl+P and Cmd/Ctrl+K open it (Cmd/Ctrl+K not over selected
//! text), titles match as you type, full-text results come from a real index after a pause, and
//! choosing a result opens the page at its block.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use gpui_component::{Root, WindowExt as _};
use leaf_core::{BlockId, PageId};
use leaf_editor::{SEARCH_DELAY_MS, SearchRow};
use leaf_search::{BlockText, PageText, PageVersion, SearchIndex};

use super::SearchPanel;
use crate::menus::{Search, SearchOrLink};
use crate::shell::Shell;

fn open_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
    cx.update(|cx| crate::init(None, cx));
    let made: Rc<RefCell<Option<Entity<Shell>>>> = Rc::default();
    let keep = made.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let shell = cx.new(|cx| Shell::new(window, cx));
        *keep.borrow_mut() = Some(shell.clone());
        Root::new(shell, window, cx)
    });
    redraw(cx);
    let shell = made.borrow().clone().unwrap();
    (shell, cx)
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A new page titled `title` (inside `parent`), its blocks holding `lines`; returns the page and
/// its blocks.
fn add(
    shell: &Entity<Shell>,
    parent: Option<PageId>,
    title: &str,
    lines: &[&str],
    cx: &mut VisualTestContext,
) -> (PageId, Vec<BlockId>) {
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| match parent {
            Some(parent) => shell.new_page_inside(parent, window, cx),
            None => shell.new_page(window, cx),
        });
    });
    cx.simulate_input(title);
    for line in lines {
        cx.simulate_keystrokes("enter"); // from the title into the first block, then a new block
        cx.simulate_input(line);
    }
    redraw(cx);
    let page = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
    let blocks = shell.update(cx, |shell, _| {
        shell
            .editor_mut()
            .open_page()
            .unwrap()
            .map(|page| page.rows().unwrap().iter().map(|row| row.id).collect())
            .unwrap_or_default()
    });
    (page, blocks)
}

fn selected(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<PageId> {
    shell.read_with(cx, |shell, _| shell.editor().selected())
}

fn dialog_open(cx: &mut VisualTestContext) -> bool {
    cx.update(Window::has_active_dialog)
}

/// A page to index: its id, title, and each block with its text.
type Indexed<'a> = (PageId, &'a str, &'a [(BlockId, &'a str)]);

/// A fresh folder holding a search index with `pages` in it.
fn index_with(name: &str, pages: &[Indexed<'_>]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("leaf-ui-search-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let (mut index, _) = SearchIndex::open(&root).unwrap();
    for (page, title, blocks) in pages {
        let text = PageText {
            page: *page,
            title: (*title).to_owned(),
            blocks: blocks
                .iter()
                .map(|(block, text)| BlockText {
                    block: *block,
                    kind: "paragraph".into(),
                    text: (*text).to_owned(),
                })
                .collect(),
        };
        index.index_page(&text, PageVersion::default()).unwrap();
    }
    index.mark_complete().unwrap();
    root
}

/// Opens the search bar the way Cmd/Ctrl+P does, but over the index in `root`.
fn open_search(shell: &Entity<Shell>, root: &Path, cx: &mut VisualTestContext) -> Entity<SearchPanel> {
    let root = root.to_owned();
    let panel = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            super::open(cx.entity().downgrade(), shell.editor(), Some(root), window, cx)
        })
    });
    redraw(cx);
    panel.unwrap()
}

/// Lets the pause pass so the full-text search runs and its answer arrives.
fn pause(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(
        u64::try_from(SEARCH_DELAY_MS).unwrap() + 10,
    ));
    cx.run_until_parked();
    redraw(cx);
}

#[gpui::test]
fn cmd_p_opens_search_and_titles_match_as_you_type(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (projects, _) = add(&shell, None, "Projects", &[], cx);
    let (design, _) = add(&shell, Some(projects), "Design notes", &[], cx);
    add(&shell, None, "Journal", &[], cx);

    cx.dispatch_action(Search);
    redraw(cx);
    assert!(dialog_open(cx));
    cx.simulate_input("desn");
    redraw(cx);
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(!dialog_open(cx), "choosing closes the search bar");
    assert_eq!(selected(&shell, cx), Some(design));
}

#[gpui::test]
fn cmd_k_opens_search_but_not_over_selected_text(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    add(&shell, None, "Notes", &["Some words"], cx);
    cx.dispatch_action(SearchOrLink);
    redraw(cx);
    assert!(dialog_open(cx), "no text selected: search");
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!dialog_open(cx));

    // Back in the page, select the last word: Cmd/Ctrl+K is the link action there.
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.focus_page(window, cx)));
    cx.simulate_keystrokes("shift-left shift-left shift-left");
    cx.dispatch_action(SearchOrLink);
    redraw(cx);
    assert!(!dialog_open(cx), "text selected: not search");
}

#[gpui::test]
fn full_text_results_open_the_page_at_the_matching_block(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (garden, blocks) = add(
        &shell,
        None,
        "Garden",
        &["Water the beds", "Tomatoes and basil", "Compost"],
        cx,
    );
    let (journal, _) = add(&shell, None, "Journal", &[], cx);
    let root = index_with(
        "full-text",
        &[
            (
                garden,
                "Garden",
                &[
                    (blocks[0], "Water the beds"),
                    (blocks[1], "Tomatoes and basil"),
                    (blocks[2], "Compost"),
                ],
            ),
            (journal, "Journal", &[]),
        ],
    );
    let panel = open_search(&shell, &root, cx);
    cx.simulate_input("basil");
    redraw(cx);
    assert!(panel.read_with(cx, |panel, _| panel.bar().searching()));
    pause(cx);
    let rows = panel.read_with(cx, |panel, _| panel.bar().rows().to_vec());
    assert!(!panel.read_with(cx, |panel, _| panel.bar().searching()));
    match rows.as_slice() {
        [SearchRow::Page(page), SearchRow::Excerpt(excerpt)] => {
            assert_eq!(page.page, garden);
            assert_eq!(excerpt.block, blocks[1]);
            assert_eq!(&excerpt.text[excerpt.highlights[0].clone()], "basil");
        }
        other => panic!("expected the page and its matching line, got {other:?}"),
    }
    assert!(
        cx.debug_bounds("search-row-1").is_some(),
        "the matching line is drawn"
    );

    cx.simulate_keystrokes("down enter");
    redraw(cx);
    assert!(!dialog_open(cx));
    assert_eq!(selected(&shell, cx), Some(garden));
    let (flashed, caret) = shell.update(cx, |shell, _| (shell.flashed_block(), shell.caret_block()));
    assert_eq!(flashed, Some(blocks[1]), "the found block is tinted");
    assert_eq!(caret, Some(blocks[1]), "the caret waits at the found block");
    let _ = std::fs::remove_dir_all(root);
}

#[gpui::test]
fn no_results_says_so_and_enter_does_nothing(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (only, _) = add(&shell, None, "Alpha", &[], cx);
    let root = index_with("none", &[(only, "Alpha", &[])]);
    open_search(&shell, &root, cx);
    cx.simulate_input("zzz");
    pause(cx);
    assert!(cx.debug_bounds("search-row-0").is_none());
    assert!(cx.debug_bounds("search-empty").is_some(), "“No results” is shown");
    cx.simulate_keystrokes("enter");
    redraw(cx);
    assert!(dialog_open(cx), "still open, nothing to open");
    assert_eq!(selected(&shell, cx), Some(only));
    let _ = std::fs::remove_dir_all(root);
}

#[gpui::test]
fn a_missing_index_still_matches_titles(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (alpha, _) = add(&shell, None, "Alpha", &[], cx);
    let nowhere = std::env::temp_dir().join(format!("leaf-ui-search-{}-missing", std::process::id()));
    let panel = open_search(&shell, &nowhere, cx);
    cx.simulate_input("alp");
    pause(cx);
    assert!(
        !panel.read_with(cx, |panel, _| panel.bar().searching()),
        "gave up quietly"
    );
    let row = cx.debug_bounds("search-row-0").unwrap().center();
    cx.simulate_click(row, Modifiers::none());
    redraw(cx);
    assert_eq!(selected(&shell, cx), Some(alpha));
    assert!(!dialog_open(cx));
}

#[gpui::test]
fn arrows_move_the_highlight_and_escape_closes(cx: &mut TestAppContext) {
    let (shell, cx) = open_window(cx);
    let (first, _) = add(&shell, None, "Alpha", &[], cx);
    add(&shell, None, "Beta", &[], cx);
    // Recent pages lead with nothing typed: Beta (opened last), then Alpha.
    cx.dispatch_action(Search);
    redraw(cx);
    cx.simulate_keystrokes("down enter");
    redraw(cx);
    assert_eq!(selected(&shell, cx), Some(first));

    cx.dispatch_action(Search);
    redraw(cx);
    cx.simulate_keystrokes("up"); // wraps to the bottom row
    cx.simulate_keystrokes("escape");
    redraw(cx);
    assert!(!dialog_open(cx));
    assert_eq!(selected(&shell, cx), Some(first), "escape opens nothing");
}
