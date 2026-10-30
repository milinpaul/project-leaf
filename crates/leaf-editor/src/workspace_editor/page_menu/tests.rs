//! Tests for the page menu model and trashing from it (E18-S04).

use leaf_model::Workspace;

use crate::{MenuPlace, PageAction, PageMenuIcon, WorkspaceEditor};

const NOW: i64 = 1_767_225_600_000; // 2026-01-01 00:00 UTC

fn actions(editor: &WorkspaceEditor, page: leaf_core::PageId, place: MenuPlace) -> Vec<PageAction> {
    editor
        .page_menu(page, place, NOW, 0)
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| entry.action)
        .collect()
}

#[test]
fn the_top_bar_and_the_sidebar_offer_the_same_actions_in_the_same_order() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    let top = actions(&editor, page, MenuPlace::TopBar);
    assert_eq!(
        top,
        [
            PageAction::ToggleFavourite,
            PageAction::Rename,
            PageAction::ToggleFullWidth,
            PageAction::MoveToTrash
        ]
    );
    let side = actions(&editor, page, MenuPlace::Sidebar);
    assert_eq!(
        side,
        [
            PageAction::AddInside,
            PageAction::ToggleFavourite,
            PageAction::Rename,
            PageAction::MoveToTrash
        ],
        "the sidebar can also add a page inside; the width is chosen where the page is open"
    );
    assert_eq!(
        actions(&editor, page, MenuPlace::Favourites),
        side,
        "a Favourites row offers what a tree row does"
    );
    let menu = editor.page_menu(page, MenuPlace::Favourites, NOW, 0).unwrap();
    assert!(menu.footer.is_empty(), "no times in a sidebar menu");
}

#[test]
fn the_favourite_entry_says_what_it_will_do() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    let favourite = |editor: &WorkspaceEditor| {
        let menu = editor.page_menu(page, MenuPlace::TopBar, NOW, 0).unwrap();
        let entry = menu.entries[0].clone();
        (entry.label, entry.icon)
    };
    assert_eq!(favourite(&editor), ("Add to Favourites", PageMenuIcon::Star));
    editor.set_favourite(page, true).unwrap();
    assert_eq!(
        favourite(&editor),
        ("Remove from Favourites", PageMenuIcon::StarOff)
    );
}

#[test]
fn move_to_trash_is_red_and_set_apart() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    let menu = editor.page_menu(page, MenuPlace::Sidebar, NOW, 0).unwrap();
    for entry in &menu.entries {
        let trash = entry.action == PageAction::MoveToTrash;
        assert_eq!(
            (entry.danger, entry.separated),
            (trash, trash),
            "{:?}",
            entry.action
        );
        assert!(entry.enabled);
        assert_eq!(entry.shortcut, None);
    }
}

#[test]
fn the_top_bar_menu_ends_with_when_the_page_was_created_and_edited() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    let three_minutes = 3 * 60_000;
    let menu = editor
        .page_menu(page, MenuPlace::TopBar, NOW + three_minutes, 60)
        .unwrap();
    assert_eq!(
        menu.footer,
        ["Created 1 Jan 2026, 01:00", "Last edited 3 minutes ago"]
    );
    let sidebar = editor.page_menu(page, MenuPlace::Sidebar, NOW, 0).unwrap();
    assert_eq!(sidebar.footer, Vec::<String>::new());
}

#[test]
fn trashing_the_open_page_opens_its_parent_else_a_neighbour() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let parent = editor.new_page(NOW).unwrap();
    let first = editor.new_page_in(Some(parent), NOW).unwrap();
    let second = editor.new_page_in(Some(parent), NOW).unwrap();
    let other = editor.new_page(NOW).unwrap();

    editor.select(second).unwrap();
    assert_eq!(editor.trash_and_open_next(second, NOW).unwrap(), Some(parent));
    assert_eq!(editor.selected(), Some(parent));

    // A top-level page has no parent: the page after it opens, else the one before.
    editor.select(parent).unwrap();
    assert_eq!(editor.trash_and_open_next(parent, NOW).unwrap(), Some(other));
    assert!(
        editor.workspace().is_in_trash(first).unwrap(),
        "its children went with it"
    );
    editor.select(other).unwrap();
    let last = editor.new_page(NOW).unwrap();
    editor.select(last).unwrap();
    assert_eq!(editor.trash_and_open_next(last, NOW).unwrap(), Some(other));

    // The last page left: nothing opens.
    assert_eq!(editor.trash_and_open_next(other, NOW).unwrap(), None);
    assert_eq!(editor.selected(), None);
}

#[test]
fn trashing_a_page_that_isnt_open_leaves_the_open_page_alone() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let open = editor.new_page(NOW).unwrap();
    let other = editor.new_page(NOW).unwrap();
    editor.select(open).unwrap();
    assert_eq!(editor.trash_and_open_next(other, NOW).unwrap(), None);
    assert_eq!(editor.selected(), Some(open));
}

#[test]
fn full_width_is_a_switch_showing_the_pages_width() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    let width = |editor: &WorkspaceEditor| {
        let menu = editor.page_menu(page, MenuPlace::TopBar, NOW, 0).unwrap();
        let entry = menu
            .entries
            .into_iter()
            .find(|entry| entry.action == PageAction::ToggleFullWidth)
            .unwrap();
        assert_eq!(
            (entry.label, entry.icon, entry.separated),
            ("Full width", PageMenuIcon::FullWidth, true)
        );
        entry.switch
    };
    assert_eq!(width(&editor), Some(false));
    editor.set_full_width(page, true).unwrap();
    assert_eq!(width(&editor), Some(true));
    let other = editor.page_menu(page, MenuPlace::TopBar, NOW, 0).unwrap();
    assert!(
        other
            .entries
            .iter()
            .filter(|entry| entry.action != PageAction::ToggleFullWidth)
            .all(|entry| entry.switch.is_none()),
        "only the width is a switch"
    );
}
