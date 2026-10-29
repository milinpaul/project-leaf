//! Tests for the Trash: listing, restoring and deleting for good.

use leaf_core::PageId;
use leaf_model::Workspace;

use super::{TrashList, trashed_ago};
use crate::WorkspaceEditor;

const NOW: i64 = 1_700_000_000_000;
const MINUTE: i64 = 60_000;

fn page(editor: &mut WorkspaceEditor, parent: Option<PageId>, title: &str) -> PageId {
    let page = editor.new_page_in(parent, NOW).unwrap();
    editor.rename(page, title).unwrap();
    page
}

fn labels(editor: &WorkspaceEditor) -> Vec<String> {
    editor.sidebar().unwrap().into_iter().map(|i| i.label).collect()
}

/// Projects (Leaf app (Design notes)), Journal, Notes.
fn sample() -> (WorkspaceEditor, [PageId; 5]) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let projects = page(&mut editor, None, "Projects");
    let leaf = page(&mut editor, Some(projects), "Leaf app");
    let design = page(&mut editor, Some(leaf), "Design notes");
    let journal = page(&mut editor, None, "Journal");
    let notes = page(&mut editor, None, "Notes");
    (editor, [projects, leaf, design, journal, notes])
}

#[test]
fn the_trash_lists_pages_newest_first_with_where_they_lived() {
    let (mut editor, [_, leaf, design, journal, _]) = sample();
    assert!(editor.trash_items().unwrap().is_empty(), "starts empty");
    editor.trash(design, NOW).unwrap();
    editor.trash(journal, NOW + MINUTE).unwrap();
    let items = editor.trash_items().unwrap();
    let summary: Vec<(&str, &str, i64)> = items
        .iter()
        .map(|i| (i.label.as_str(), i.path.as_str(), i.trashed_at))
        .collect();
    assert_eq!(
        summary,
        [
            ("Journal", "", NOW + MINUTE),
            ("Design notes", "Projects / Leaf app", NOW)
        ]
    );
    editor.trash(leaf, NOW + 2 * MINUTE).unwrap();
    assert_eq!(
        editor.trash_items().unwrap().len(),
        3,
        "a page inside a trashed one stays listed"
    );
}

#[test]
fn restoring_puts_a_page_back_or_at_the_top_level_if_its_parent_is_binned() {
    let (mut editor, [projects, leaf, design, _, _]) = sample();
    editor.trash(design, NOW).unwrap();
    editor.restore(design).unwrap();
    assert!(
        editor.workspace().children(leaf).unwrap().contains(&design),
        "back where it was"
    );

    editor.trash(design, NOW).unwrap();
    editor.trash(projects, NOW).unwrap();
    editor.restore(design).unwrap();
    assert_eq!(
        labels(&editor),
        ["Journal", "Notes", "Design notes"],
        "parent still binned"
    );
    assert!(editor.restore(design).is_err(), "not in the trash any more");
}

#[test]
fn deleting_for_good_returns_every_page_and_forgets_them() {
    let (mut editor, [projects, leaf, design, journal, _]) = sample();
    editor.select(design).unwrap();
    editor.open_page().unwrap(); // its blocks are in memory
    editor.trash(projects, NOW).unwrap();
    editor.select(design).unwrap(); // opened from the Trash view
    assert!(editor.selected_in_trash().unwrap());

    let mut deleted = editor.delete_forever(projects).unwrap();
    deleted.sort();
    let mut expected = vec![projects, leaf, design];
    expected.sort();
    assert_eq!(deleted, expected);
    assert_eq!(editor.selected(), None, "the open page went");
    assert!(editor.page_editor(design).is_none(), "its blocks are forgotten");
    assert!(!editor.recent_pages().contains(&design));
    assert!(!editor.expanded_pages().contains(&projects));
    assert!(!editor.workspace().contains(design));
    assert!(!editor.selected_in_trash().unwrap());
    assert!(editor.delete_forever(journal).is_err(), "only pages in the trash");
}

#[test]
fn emptying_the_trash_deletes_everything_in_it_once() {
    let (mut editor, [projects, leaf, design, journal, _]) = sample();
    editor.trash(design, NOW).unwrap();
    editor.trash(projects, NOW + 1).unwrap(); // design is inside it, and listed on its own too
    editor.trash(journal, NOW + 2).unwrap();
    let mut deleted = editor.empty_trash().unwrap();
    deleted.sort();
    let mut expected = vec![projects, leaf, design, journal];
    expected.sort();
    assert_eq!(deleted, expected);
    assert!(editor.trash_items().unwrap().is_empty(), "the trash is empty");
    assert_eq!(labels(&editor), ["Notes"]);
    assert!(editor.empty_trash().unwrap().is_empty(), "nothing left");
}

#[test]
fn the_list_filters_titles_and_counts_everything() {
    let (mut editor, [_, _, design, journal, notes]) = sample();
    for (i, page) in [design, journal, notes].into_iter().enumerate() {
        editor.trash(page, NOW + i64::try_from(i).unwrap()).unwrap();
    }
    let mut list = TrashList::new(editor.trash_items().unwrap());
    let titles = |list: &TrashList| {
        list.shown()
            .map(|(item, _)| item.label.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles(&list), ["Notes", "Journal", "Design notes"]);
    list.set_query("notes");
    assert_eq!(titles(&list), ["Notes", "Design notes"], "best match first");
    let highlight: Vec<_> = list.shown().map(|(_, ranges)| ranges.to_vec()).collect();
    assert_eq!(highlight[1], vec![7..12]);
    assert_eq!((list.query(), list.total()), ("notes", 3));
    list.set_query(" ");
    assert_eq!(titles(&list).len(), 3);
}

#[test]
fn trashed_times_read_naturally() {
    let ago = |ms: i64| trashed_ago(NOW, NOW - ms);
    assert_eq!(ago(5_000), "just now");
    assert_eq!(ago(5 * MINUTE), "5 min ago");
    assert_eq!(ago(3 * 60 * MINUTE), "3 h ago");
    assert_eq!(ago(30 * 60 * MINUTE), "yesterday");
    assert_eq!(ago(4 * 24 * 60 * MINUTE), "4 days ago");
    assert_eq!(ago(43 * 24 * 60 * MINUTE), "6 weeks ago");
    assert_eq!(
        trashed_ago(NOW, NOW + MINUTE),
        "just now",
        "a clock that moved back"
    );
}
