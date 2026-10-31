//! A database inside a page in a (test) window (E09-S09): `/database` adds it, listed in the sidebar under its page;
//! its table edits like the full page's without the page's keys getting in; deleting its block trashes it, and undo
//! brings it back.

use gpui::{Entity, Modifiers, VisualTestContext};
use leaf_core::PageId;
use leaf_model::Caret;

use super::{draw, editing, open_window};
use crate::shell::Shell;
use crate::shell::database_page::EditKind;

fn click_on(cx: &mut VisualTestContext, selector: String) {
    let selector: &'static str = Box::leak(selector.into_boxed_str());
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    draw(cx);
}

fn drawn(cx: &mut VisualTestContext, selector: String) -> bool {
    cx.debug_bounds(Box::leak(selector.into_boxed_str())).is_some()
}

/// A new page with the keyboard on its first line, then `/database` chosen there. Returns the page and the
/// database's page.
fn page_with_database(
    cx: &mut gpui::TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext, PageId, PageId) {
    let (shell, cx) = open_window(cx);
    cx.update(|window, cx| shell.update(cx, |shell, cx| shell.new_page(window, cx)));
    let host = shell.read_with(cx, |shell, _| shell.editor().selected().unwrap());
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            let block = shell.page().unwrap().ensure_block().unwrap();
            shell.page().unwrap().set_caret(Caret { block, at: 0 }).unwrap();
            shell.focus_page(window, cx);
        });
    });
    draw(cx);
    cx.simulate_input("/database");
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    let databases = shell.update(cx, |shell, _| shell.page().unwrap().inline_databases().unwrap());
    assert_eq!(databases.len(), 1, "one database on the page");
    (shell, cx, host, databases[0])
}

fn page_text(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.update(cx, |shell, _| {
        shell
            .page()
            .unwrap()
            .rows()
            .unwrap()
            .into_iter()
            .filter_map(|row| row.text)
            .collect()
    })
}

fn trashed(shell: &Entity<Shell>, cx: &mut VisualTestContext, page: PageId) -> bool {
    shell.read_with(cx, |shell, _| {
        shell.editor().workspace().is_in_trash(page).unwrap()
    })
}

#[gpui::test]
fn slash_database_adds_a_table_to_the_page_listed_under_it_in_the_sidebar(cx: &mut gpui::TestAppContext) {
    let (shell, cx, host, db) = page_with_database(cx);
    assert!(drawn(cx, format!("inline-database-{db}")), "drawn in the page");
    assert!(
        cx.debug_bounds("database-head-0").is_some(),
        "with its title column"
    );
    assert!(
        cx.debug_bounds("database-head-1").is_some(),
        "and the starter Tags column"
    );
    assert!(drawn(cx, format!("inline-database-new-row-{db}")));
    shell.read_with(cx, |shell, _| {
        let editor = shell.editor();
        assert_eq!(editor.workspace().parent(db).unwrap(), Some(host));
        let sidebar = editor.sidebar().unwrap();
        assert!(
            sidebar.iter().any(|row| row.page == db && row.depth == 1),
            "the sidebar lists it under its page"
        );
        assert_eq!(editor.selected(), Some(host), "the page stays open");
    });
    assert_eq!(
        page_text(&shell, cx),
        vec![String::new()],
        "`/database` is gone, a line waits below"
    );
}

#[gpui::test]
fn the_inline_table_edits_like_the_full_page_and_the_page_keys_stay_out(cx: &mut gpui::TestAppContext) {
    let (shell, cx, _, db) = page_with_database(cx);
    cx.simulate_input("Plan");
    draw(cx);
    assert_eq!(page_text(&shell, cx), vec!["Plan".to_owned()]);

    click_on(cx, format!("inline-database-new-row-{db}"));
    assert_eq!(
        editing(&shell, cx),
        Some(EditKind::Title),
        "typing the new row's title"
    );
    cx.simulate_input("Buy seeds");
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(editing(&shell, cx), None);
    let rows = shell.read_with(cx, |shell, cx| {
        let table = shell.database_view().unwrap().table.clone();
        let titles: Vec<String> = table
            .read(cx)
            .delegate()
            .view
            .rows
            .iter()
            .map(|r| r.title.clone())
            .collect();
        titles
    });
    assert_eq!(rows, vec!["Buy seeds".to_owned()]);

    // The table has the keyboard: the page's keys do nothing to the page.
    cx.simulate_keystrokes("backspace secondary-a secondary-b");
    draw(cx);
    assert_eq!(
        page_text(&shell, cx),
        vec!["Plan".to_owned()],
        "the page's text is untouched"
    );
    let selected = shell.update(cx, |shell, _| shell.page().unwrap().selection().unwrap());
    assert_eq!(selected, None, "no blocks selected");
}

#[gpui::test]
fn deleting_the_block_trashes_the_database_and_undo_brings_it_back(cx: &mut gpui::TestAppContext) {
    let (shell, cx, _, db) = page_with_database(cx);
    // Within the merge interval the delete would undo together with making the database.
    shell.update(cx, |shell, _| shell.page().unwrap().set_undo_merge_interval(0));
    click_on(cx, format!("inline-database-title-{db}"));
    cx.simulate_keystrokes("backspace");
    draw(cx);
    assert!(trashed(&shell, cx, db), "the database went to the Trash");
    assert!(!drawn(cx, format!("inline-database-{db}")));
    let in_trash = shell.read_with(cx, |shell, _| {
        shell
            .editor()
            .trash_items()
            .unwrap()
            .iter()
            .any(|item| item.page == db)
    });
    assert!(in_trash, "listed in the Trash");

    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    assert!(!trashed(&shell, cx, db), "undo brings it back");
    assert!(drawn(cx, format!("inline-database-{db}")));
}
