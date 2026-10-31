//! The backlinks line and list in a (test) window (E10-S07). An in-memory workspace has no index, so the
//! tests set what the index would have found.

use gpui::{Modifiers, TestAppContext, VisualTestContext};
use leaf_core::{BlockId, PageId};
use leaf_editor::BacklinkSource;
use leaf_model::Caret;

use crate::shell::Shell;

const NOW: i64 = 1_700_000_000_000;

/// "Budget" (with a block) and "Trip to Japan"; Trip to Japan is open.
fn open(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, &mut VisualTestContext, [PageId; 2], BlockId) {
    cx.update(|cx| crate::init(None, cx));
    let (shell, cx) = cx.add_window_view(Shell::new);
    let budget = shell.update(cx, |shell, _| {
        let budget = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(budget, "Budget").unwrap();
        budget
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(budget, window, cx)));
    let block = shell.update(cx, |shell, _| {
        let page = shell.editor.open_page().unwrap().unwrap();
        let first = page.ensure_block().unwrap();
        page.set_text(first, "first").unwrap();
        let second = page.enter(first).unwrap();
        page.set_text(second, "Flights are booked").unwrap();
        second
    });
    let japan = shell.update(cx, |shell, _| {
        let japan = shell.editor.new_page(NOW).unwrap();
        shell.editor.rename(japan, "Trip to Japan").unwrap();
        japan
    });
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(japan, window, cx)));
    draw(cx);
    (shell, cx, [japan, budget], block)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn found(shell: &gpui::Entity<Shell>, cx: &mut VisualTestContext, page: PageId, block: BlockId) {
    shell.update(cx, |shell, _| {
        shell.blocks.backlinks.found = vec![BacklinkSource {
            page,
            block,
            before: "Flights for ".into(),
            after: " are booked".into(),
        }];
    });
    draw(cx);
}

#[gpui::test]
fn nothing_shows_without_backlinks(cx: &mut TestAppContext) {
    let (_, cx, ..) = open(cx);
    assert!(cx.debug_bounds("page-header").is_some());
    assert!(cx.debug_bounds("backlinks-toggle").is_none());
}

#[gpui::test]
fn the_line_opens_the_list_and_an_entry_opens_the_page_at_its_block(cx: &mut TestAppContext) {
    let (shell, cx, [_, budget], block) = open(cx);
    found(&shell, cx, budget, block);
    let toggle = cx
        .debug_bounds("backlinks-toggle")
        .expect("1 backlink under the title");
    let title = cx.debug_bounds("page-header").unwrap();
    assert!(toggle.top() > title.top(), "under the title");
    assert!(cx.debug_bounds("backlinks-list").is_none(), "closed at first");

    cx.simulate_click(toggle.center(), Modifiers::none());
    draw(cx);
    let entry = cx.debug_bounds("backlink-0").expect("the list, open").center();
    cx.simulate_click(entry, Modifiers::none());
    draw(cx);
    let (selected, caret) = shell.update(cx, |shell, _| {
        let selected = shell.editor.selected();
        (
            selected,
            shell.editor.open_page().unwrap().unwrap().caret().unwrap(),
        )
    });
    assert_eq!(selected, Some(budget));
    assert_eq!(
        caret,
        Some(Caret { block, at: 0 }),
        "at the linking block, as a search result opens"
    );
}

#[gpui::test]
fn another_page_starts_with_the_list_closed(cx: &mut TestAppContext) {
    let (shell, cx, [japan, budget], block) = open(cx);
    found(&shell, cx, budget, block);
    let toggle = cx.debug_bounds("backlinks-toggle").unwrap().center();
    cx.simulate_click(toggle, Modifiers::none());
    draw(cx);
    assert!(shell.read_with(cx, |shell, _| shell.blocks.backlinks.open));
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(budget, window, cx)));
    draw(cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.open(japan, window, cx)));
    draw(cx);
    assert!(cx.debug_bounds("backlinks-list").is_none());
}
