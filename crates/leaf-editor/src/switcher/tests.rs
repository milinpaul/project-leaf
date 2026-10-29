//! Tests for the quick switcher.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use leaf_core::PageId;
use leaf_model::{Workspace, WorkspaceCommand};

use super::{MAX_SWITCHER_RESULTS, QuickSwitcher, SwitcherResult};
use crate::{Vertical, WorkspaceEditor};

const NOW: i64 = 1_700_000_000_000;

fn page(editor: &mut WorkspaceEditor, parent: Option<PageId>, title: &str) -> PageId {
    let page = editor.new_page_in(parent, NOW).unwrap();
    editor.rename(page, title).unwrap();
    page
}

/// A small workspace: Projects (Leaf app (Design notes, Release checklist), Garden plan), Journal, Notes.
fn sample() -> (WorkspaceEditor, Vec<PageId>) {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let projects = page(&mut editor, None, "Projects");
    let leaf = page(&mut editor, Some(projects), "Leaf app");
    let design = page(&mut editor, Some(leaf), "Design notes");
    let release = page(&mut editor, Some(leaf), "Release checklist");
    let garden = page(&mut editor, Some(projects), "Garden plan");
    let journal = page(&mut editor, None, "Journal");
    let notes = page(&mut editor, None, "Notes");
    // The editor offers no icon picker yet, so set the icon on a copy of the workspace.
    let mut workspace = Workspace::from_bytes(&editor.workspace().to_bytes().unwrap()).unwrap();
    workspace
        .apply(WorkspaceCommand::SetPageIcon {
            page: journal,
            icon: Some("📓".into()),
        })
        .unwrap();
    let editor = WorkspaceEditor::new(workspace);
    (
        editor,
        vec![projects, leaf, design, release, garden, journal, notes],
    )
}

/// Results as "icon title [matched] — path" lines.
fn render(results: &[SwitcherResult]) -> String {
    let mut out = String::new();
    for result in results {
        let mut title = String::new();
        let mut at = 0;
        for range in &result.highlights {
            title.push_str(&result.label[at..range.start]);
            write!(title, "[{}]", &result.label[range.clone()]).unwrap();
            at = range.end;
        }
        title.push_str(&result.label[at..]);
        let icon = result.icon.as_deref().unwrap_or("·");
        writeln!(out, "{icon} {title}  —  {}", result.path).unwrap();
    }
    out
}

fn labels(switcher: &QuickSwitcher) -> Vec<&str> {
    switcher.results().iter().map(|r| r.label.as_str()).collect()
}

#[test]
fn typing_ranks_titles_and_shows_where_each_page_lives() {
    let (editor, _) = sample();
    let mut switcher = QuickSwitcher::open(editor.workspace(), &[]).unwrap();
    switcher.set_query("notes");
    insta::assert_snapshot!(render(switcher.results()), @r"
    · [Notes]  —  
    · Design [notes]  —  Projects / Leaf app
    ");
    switcher.set_query("lf");
    insta::assert_snapshot!(render(switcher.results()), @"· [L]ea[f] app  —  Projects");
    switcher.set_query("gar pl");
    insta::assert_snapshot!(render(switcher.results()), @"· [Gar]den [pl]an  —  Projects");
}

#[test]
fn an_empty_query_lists_recent_pages_first_then_the_rest_in_sidebar_order() {
    let (mut editor, pages) = sample();
    let (projects, release, journal) = (pages[0], pages[3], pages[5]);
    editor.select(release).unwrap();
    editor.select(journal).unwrap();
    let switcher = QuickSwitcher::open(editor.workspace(), editor.recent_pages()).unwrap();
    assert_eq!(
        switcher.chosen(),
        Some(journal),
        "the last page opened comes first"
    );
    let order: Vec<PageId> = switcher.results().iter().map(|r| r.page).collect();
    assert_eq!(
        &order[..3],
        &[journal, release, projects],
        "recent first, then sidebar order"
    );
    assert!(order.contains(&projects));
    assert_eq!(order.len(), pages.len(), "every page once");
}

#[test]
fn up_and_down_wrap_and_enter_picks_the_highlighted_page() {
    let (editor, pages) = sample();
    let mut switcher = QuickSwitcher::open(editor.workspace(), &[]).unwrap();
    switcher.set_query("p");
    let count = switcher.results().len();
    assert!(count >= 3);
    assert_eq!(switcher.selected(), Some(0));
    switcher.move_selection(Vertical::Up);
    assert_eq!(
        switcher.selected(),
        Some(count - 1),
        "up from the top wraps to the bottom"
    );
    switcher.move_selection(Vertical::Down);
    assert_eq!(switcher.selected(), Some(0), "and back");
    switcher.select(2);
    assert_eq!(switcher.chosen(), Some(switcher.results()[2].page));
    switcher.select(99);
    assert_eq!(switcher.selected(), Some(2), "a row that isn't there is ignored");

    switcher.set_query("pro");
    assert_eq!(
        switcher.selected(),
        Some(0),
        "a new query starts at the best match"
    );
    assert_eq!(switcher.chosen(), Some(pages[0]));
}

#[test]
fn no_match_means_nothing_to_choose() {
    let (editor, _) = sample();
    let mut switcher = QuickSwitcher::open(editor.workspace(), &[]).unwrap();
    switcher.set_query("zebra");
    assert!(switcher.results().is_empty(), "nothing matches");
    assert_eq!((switcher.selected(), switcher.chosen()), (None, None));
    switcher.move_selection(Vertical::Down);
    assert_eq!(switcher.selected(), None);
    assert_eq!(switcher.query(), "zebra");
}

#[test]
fn pages_in_the_trash_and_their_children_are_left_out() {
    let (mut editor, pages) = sample();
    editor.select(pages[3]).unwrap();
    editor.trash(pages[1], NOW).unwrap(); // Leaf app, with Design notes and Release checklist
    let mut switcher = QuickSwitcher::open(editor.workspace(), editor.recent_pages()).unwrap();
    assert!(
        !switcher.results().iter().any(|r| r.page == pages[3]),
        "not even as a recent page"
    );
    switcher.set_query("notes");
    assert_eq!(labels(&switcher), ["Notes"]);
}

#[test]
fn untitled_pages_are_findable_by_their_placeholder() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    editor.new_page(NOW).unwrap();
    let mut switcher = QuickSwitcher::open(editor.workspace(), &[]).unwrap();
    switcher.set_query("untit");
    assert_eq!(labels(&switcher), ["Untitled"]);
}

#[test]
fn recent_pages_are_capped_and_move_to_the_front_when_reopened() {
    let (mut editor, pages) = sample();
    editor.select(pages[0]).unwrap();
    editor.select(pages[1]).unwrap();
    editor.select(pages[0]).unwrap();
    assert_eq!(&editor.recent_pages()[..2], &[pages[0], pages[1]]);
    for _ in 0..30 {
        editor.new_page(NOW).unwrap();
    }
    assert_eq!(editor.recent_pages().len(), crate::RECENT_LIMIT);
}

#[test]
fn opening_and_typing_with_5000_pages_is_quick() {
    let mut workspace = Workspace::new();
    let mut parents: Vec<Option<PageId>> = vec![None];
    for i in 0..5_000usize {
        let parent = parents[i % parents.len().min(40)];
        let index = workspace.child_slots(parent).unwrap().len();
        let leaf_model::Outcome::PageCreated(page) = workspace
            .apply(WorkspaceCommand::CreatePage {
                parent,
                index,
                title: format!("Meeting notes {i} about project {}", i % 97),
                created_at: NOW,
            })
            .unwrap()
        else {
            unreachable!()
        };
        parents.push(Some(page));
    }
    // Best of three, so a busy CI machine doesn't fail it. The budget is for an unoptimised test build
    // (about 55 ms here); an optimised build takes about 13 ms, inside the story's 50 ms.
    let best = (0..3)
        .map(|_| {
            let started = Instant::now();
            let mut switcher = QuickSwitcher::open(&workspace, &[]).unwrap();
            switcher.set_query("mtg 42");
            assert_eq!(switcher.results().len(), MAX_SWITCHER_RESULTS);
            started.elapsed()
        })
        .min()
        .unwrap_or_default();
    assert!(best < Duration::from_millis(150), "took {best:?}");
}
