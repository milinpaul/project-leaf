//! Tests for the sidebar page tree: rows, expanding, adding pages inside, trashing and moving.

use std::time::{Duration, Instant};

use leaf_core::PageId;
use leaf_model::Workspace;
use proptest::prelude::*;

use super::DropPlace;
use crate::WorkspaceEditor;

const NOW: i64 = 1_700_000_000_000;

/// A page titled `title` inside `parent`, made through the editor as the sidebar would.
fn page(editor: &mut WorkspaceEditor, parent: Option<PageId>, title: &str) -> PageId {
    let page = editor.new_page_in(parent, NOW).unwrap();
    editor.rename(page, title).unwrap();
    page
}

/// The sidebar as "  title" lines, two spaces per level, with `+`/`-` for collapsed/expanded parents.
fn outline(editor: &WorkspaceEditor) -> Vec<String> {
    editor
        .sidebar()
        .unwrap()
        .into_iter()
        .map(|item| {
            let mark = match (item.has_children, item.expanded) {
                (false, _) => "",
                (true, false) => "+",
                (true, true) => "-",
            };
            format!("{}{mark}{}", "  ".repeat(item.depth), item.label)
        })
        .collect()
}

/// `A` (with `A1`, `A2` inside), then `B`, all expanded.
fn sample() -> (WorkspaceEditor, [PageId; 4]) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let a = page(&mut editor, None, "A");
    let a1 = page(&mut editor, Some(a), "A1");
    let a2 = page(&mut editor, Some(a), "A2");
    let b = page(&mut editor, None, "B");
    (editor, [a, a1, a2, b])
}

#[test]
fn children_show_under_an_expanded_page_and_hide_when_collapsed() {
    let (mut editor, [a, ..]) = sample();
    assert_eq!(outline(&editor), ["-A", "  A1", "  A2", "B"]);
    editor.toggle_expanded(a);
    assert_eq!(outline(&editor), ["+A", "B"]);
    assert!(!editor.is_expanded(a));
    editor.toggle_expanded(a);
    assert_eq!(outline(&editor), ["-A", "  A1", "  A2", "B"]);
}

#[test]
fn adding_a_page_inside_appends_it_opens_the_parent_and_selects_it() {
    let (mut editor, [a, ..]) = sample();
    editor.set_expanded(a, false);
    let a3 = editor.new_page_in(Some(a), NOW).unwrap();
    assert_eq!(editor.selected(), Some(a3));
    assert_eq!(outline(&editor), ["-A", "  A1", "  A2", "  Untitled", "B"]);
    let row = editor
        .sidebar()
        .unwrap()
        .into_iter()
        .find(|i| i.page == a3)
        .unwrap();
    assert!(row.selected && row.depth == 1);
}

#[test]
fn expanded_pages_are_saved_without_deleted_ones_and_restored() {
    let (mut editor, [a, a1, ..]) = sample();
    assert!(
        editor.set_expanded(a1, true),
        "expanding a page counts as a change"
    );
    assert!(!editor.set_expanded(a1, true), "expanding it again does not");
    let mut expected = vec![a, a1];
    expected.sort();
    assert_eq!(editor.expanded_pages(), expected);

    let bytes = editor.workspace().to_bytes().unwrap();
    let mut reopened = WorkspaceEditor::new(Workspace::from_bytes(&bytes).unwrap());
    assert_eq!(outline(&reopened), ["+A", "B"], "collapsed until restored");
    reopened.restore_expanded([a, PageId::new()]);
    assert_eq!(reopened.expanded_pages(), vec![a], "unknown pages are dropped");
    assert_eq!(outline(&reopened), ["-A", "  A1", "  A2", "B"]);
}

#[test]
fn trashing_hides_the_page_and_its_children_and_closes_it_if_open() {
    let (mut editor, [a, a1, _, b]) = sample();
    editor.select(a1).unwrap();
    editor.trash(b, NOW).unwrap();
    assert_eq!(outline(&editor), ["-A", "  A1", "  A2"]);
    assert_eq!(
        editor.selected(),
        Some(a1),
        "another page's trash leaves this one open"
    );
    editor.trash(a, NOW).unwrap();
    assert_eq!(outline(&editor), Vec::<String>::new());
    assert_eq!(editor.selected(), None, "the open page went with its parent");
    assert!(editor.trash(a, NOW).is_err(), "already in the trash");
}

#[test]
fn the_pointer_position_picks_before_inside_or_after() {
    let page = PageId::new();
    assert_eq!(DropPlace::at(page, 2.0, 28.0), DropPlace::Before(page));
    assert_eq!(DropPlace::at(page, 14.0, 28.0), DropPlace::Inside(page));
    assert_eq!(DropPlace::at(page, 26.0, 28.0), DropPlace::After(page));
    assert_eq!(DropPlace::at(page, 26.0, 28.0).target(), Some(page));
    assert_eq!(DropPlace::End.target(), None);
}

#[test]
fn moving_reorders_nests_and_unnests() {
    let (mut editor, [a, a1, a2, b]) = sample();
    assert!(editor.move_page(b, DropPlace::Before(a)).unwrap());
    assert_eq!(outline(&editor), ["B", "-A", "  A1", "  A2"]);
    assert!(editor.move_page(a1, DropPlace::After(a2)).unwrap());
    assert_eq!(outline(&editor), ["B", "-A", "  A2", "  A1"]);
    assert!(editor.move_page(a2, DropPlace::Inside(b)).unwrap());
    assert_eq!(
        outline(&editor),
        ["-B", "  A2", "-A", "  A1"],
        "B opens to show it"
    );
    assert!(editor.move_page(a2, DropPlace::After(a)).unwrap());
    assert_eq!(
        outline(&editor),
        ["B", "-A", "  A2", "  A1"],
        "below an expanded page with children means its first child"
    );
    editor.set_expanded(a, false);
    assert!(editor.move_page(b, DropPlace::After(a)).unwrap());
    assert_eq!(
        outline(&editor),
        ["+A", "B"],
        "below a collapsed page means beside it"
    );
    editor.set_expanded(a, true);
    assert!(editor.move_page(a1, DropPlace::End).unwrap());
    assert_eq!(
        outline(&editor),
        ["-A", "  A2", "B", "A1"],
        "the space below the tree is the top level's end"
    );
    assert!(!editor.move_page(a1, DropPlace::End).unwrap(), "already last");
}

#[test]
fn a_page_cannot_be_dropped_on_or_inside_itself() {
    let (mut editor, [a, a1, _, b]) = sample();
    let a1x = page(&mut editor, Some(a1), "A1x");
    let before = editor.workspace().to_bytes().unwrap();
    for place in [
        DropPlace::Inside(a),
        DropPlace::Before(a),
        DropPlace::Inside(a1),
        DropPlace::After(a1x),
    ] {
        assert!(!editor.can_drop(a, place).unwrap(), "{place:?}");
        assert!(!editor.move_page(a, place).unwrap(), "{place:?}");
    }
    assert_eq!(
        editor.workspace().to_bytes().unwrap(),
        before,
        "nothing was written"
    );
    assert!(editor.can_drop(a1, DropPlace::Inside(b)).unwrap());
}

#[test]
fn dropping_a_page_where_it_already_is_changes_nothing() {
    let (mut editor, [a, a1, a2, _]) = sample();
    let before = editor.workspace().to_bytes().unwrap();
    assert!(!editor.move_page(a1, DropPlace::Before(a2)).unwrap());
    assert!(!editor.move_page(a2, DropPlace::After(a1)).unwrap());
    assert!(
        !editor.move_page(a2, DropPlace::Inside(a)).unwrap(),
        "already its last child"
    );
    assert_eq!(editor.workspace().to_bytes().unwrap(), before);
}

#[test]
fn drops_land_where_shown_even_with_trashed_pages_in_between() {
    // Model positions count trashed pages; the sidebar does not.
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let a = page(&mut editor, None, "A");
    let gone = page(&mut editor, None, "Gone");
    let b = page(&mut editor, None, "B");
    let c = page(&mut editor, None, "C");
    editor.trash(gone, NOW).unwrap();
    assert!(editor.move_page(c, DropPlace::After(a)).unwrap());
    assert_eq!(outline(&editor), ["A", "C", "B"]);
    assert!(editor.move_page(a, DropPlace::Before(b)).unwrap());
    assert_eq!(outline(&editor), ["C", "A", "B"]);
    assert!(editor.move_page(c, DropPlace::After(b)).unwrap());
    assert_eq!(outline(&editor), ["A", "B", "C"]);
}

#[test]
fn renaming_any_page_changes_only_that_page() {
    let (mut editor, [a, a1, ..]) = sample();
    editor.select(a).unwrap();
    editor.rename(a1, "First").unwrap();
    assert_eq!(outline(&editor), ["-A", "  First", "  A2", "B"]);
    let before = editor.workspace().to_bytes().unwrap();
    editor.rename(a1, "First").unwrap();
    assert_eq!(
        editor.workspace().to_bytes().unwrap(),
        before,
        "same title, no change"
    );
}

/// 500 pages, six levels deep, every page expanded: the size the story asks to stay smooth.
fn big_tree() -> WorkspaceEditor {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let mut last_at_depth: Vec<PageId> = Vec::new();
    for i in 0..500usize {
        let depth = i % 6;
        let parent = depth.checked_sub(1).map(|up| last_at_depth[up]);
        let page = page(&mut editor, parent, &format!("Page {i}"));
        last_at_depth.truncate(depth);
        last_at_depth.push(page);
    }
    editor
}

#[test]
fn listing_500_pages_six_levels_deep_is_quick() {
    let editor = big_tree();
    let rows = editor.sidebar().unwrap();
    assert_eq!(rows.len(), 500, "everything expanded is listed");
    assert_eq!(rows.iter().map(|row| row.depth).max(), Some(5));
    // The sidebar is rebuilt on every redraw. Best of five keeps a busy CI machine from failing it;
    // the budget is for an unoptimised test build (a release build is many times faster).
    let best = (0..5)
        .map(|_| {
            let started = Instant::now();
            editor.sidebar().unwrap();
            started.elapsed()
        })
        .min()
        .unwrap_or_default();
    assert!(best < Duration::from_millis(50), "took {best:?}");
}

/// A random edit to the tree: move page `page` to a place next to or into page `target`.
#[derive(Debug, Clone)]
enum Step {
    Move { page: usize, target: usize, place: u8 },
    Trash { page: usize },
    Toggle { page: usize },
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        6 => (0..12usize, 0..12usize, 0..4u8).prop_map(|(page, target, place)| Step::Move { page, target, place }),
        1 => (0..12usize).prop_map(|page| Step::Trash { page }),
        2 => (0..12usize).prop_map(|page| Step::Toggle { page }),
    ]
}

proptest! {
    /// Whatever was moved, it ends up exactly where the drop line was drawn.
    #[test]
    fn every_allowed_drop_lands_where_it_was_aimed(steps in prop::collection::vec(step(), 1..40)) {
        let mut editor = WorkspaceEditor::new(Workspace::new());
        let mut pages = Vec::new();
        for i in 0..12 {
            let parent = (i % 3 != 0).then(|| pages.last().copied()).flatten();
            pages.push(page(&mut editor, parent, &format!("P{i}")));
        }
        for step in steps {
            match step {
                Step::Trash { page } => {
                    let page = pages[page];
                    if !editor.workspace().is_in_trash(page).unwrap() {
                        editor.trash(page, NOW).unwrap();
                    }
                }
                Step::Toggle { page } => editor.toggle_expanded(pages[page]),
                Step::Move { page, target, place } => {
                    let (page, target) = (pages[page], pages[target]);
                    let ws = editor.workspace();
                    if ws.is_in_trash(page).unwrap() || ws.is_in_trash(target).unwrap() {
                        continue;
                    }
                    let place = match place {
                        0 => DropPlace::Before(target),
                        1 => DropPlace::After(target),
                        2 => DropPlace::Inside(target),
                        _ => DropPlace::End,
                    };
                    let into_first = matches!(place, DropPlace::After(_))
                        && editor.is_expanded(target)
                        && !editor.workspace().children(target).unwrap().is_empty();
                    if !editor.can_drop(page, place).unwrap() {
                        prop_assert!(!editor.move_page(page, place).unwrap());
                        continue;
                    }
                    editor.move_page(page, place).unwrap();
                    let ws = editor.workspace();
                    let siblings = |parent: Option<PageId>| match parent {
                        Some(parent) => ws.children(parent).unwrap(),
                        None => ws.root_pages().unwrap(),
                    };
                    let parent = ws.parent(page).unwrap();
                    let list = siblings(parent);
                    let at = list.iter().position(|&p| p == page).unwrap();
                    match place {
                        DropPlace::Inside(target) => {
                            prop_assert_eq!(parent, Some(target));
                            prop_assert_eq!(at, list.len() - 1, "last child");
                        }
                        DropPlace::After(target) if into_first => {
                            prop_assert_eq!(parent, Some(target));
                            prop_assert_eq!(at, 0, "first child");
                        }
                        DropPlace::After(target) => {
                            prop_assert_eq!(parent, ws.parent(target).unwrap());
                            prop_assert_eq!(list[at - 1], target);
                        }
                        DropPlace::Before(target) => {
                            prop_assert_eq!(parent, ws.parent(target).unwrap());
                            prop_assert_eq!(list[at + 1], target);
                        }
                        DropPlace::End => {
                            prop_assert_eq!(parent, None);
                            prop_assert_eq!(at, list.len() - 1, "last top-level page");
                        }
                    }
                }
            }
        }
        // The rows always match a walk of the visible tree.
        let rows = editor.sidebar().unwrap();
        prop_assert!(rows.windows(2).all(|pair| pair[1].depth <= pair[0].depth + 1));
    }
}

#[test]
fn moving_or_trashing_unknown_pages_is_an_error() {
    let (mut editor, [a, ..]) = sample();
    let ghost = PageId::new();
    assert!(editor.trash(ghost, NOW).is_err());
    assert!(editor.can_drop(a, DropPlace::Inside(ghost)).is_err());
    assert!(editor.new_page_in(Some(ghost), NOW).is_err());
    assert!(editor.rename(ghost, "x").is_err());
    // A page in the trash can't take new pages or moves until restored.
    editor.trash(a, NOW).unwrap();
    assert!(editor.new_page_in(Some(a), NOW).is_err());
    let b = editor.new_page(NOW).unwrap();
    assert!(editor.move_page(b, DropPlace::Inside(a)).is_err());
}
