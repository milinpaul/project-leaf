//! Tests for the top bar's breadcrumb (E18-S03).

use leaf_core::PageId;
use leaf_model::Workspace;

use crate::{Crumb, MAX_CRUMBS, UNTITLED, WorkspaceEditor};

const NOW: i64 = 1_700_000_000_000;

/// The breadcrumb as text: titles joined with " / ", the gap as "…".
fn shown(editor: &WorkspaceEditor, page: PageId) -> String {
    editor
        .breadcrumb(page)
        .unwrap()
        .into_iter()
        .map(|crumb| match crumb {
            Crumb::Page { label, icon, .. } => icon.map_or(label.clone(), |icon| format!("{icon} {label}")),
            Crumb::Gap => "…".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

/// A chain of nested pages titled A, B, C, …; returns them from the top.
fn chain(editor: &mut WorkspaceEditor, depth: usize) -> Vec<PageId> {
    let mut pages: Vec<PageId> = Vec::new();
    for (i, title) in ["A", "B", "C", "D", "E", "F"].into_iter().take(depth).enumerate() {
        let page = editor.new_page_in(pages.last().copied(), NOW).unwrap();
        editor.rename(page, title).unwrap();
        assert_eq!(pages.len(), i);
        pages.push(page);
    }
    pages
}

#[test]
fn a_top_level_page_is_just_itself() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    assert_eq!(shown(&editor, page), UNTITLED);
    editor.rename(page, "Notes").unwrap();
    editor.set_icon(page, Some("📝")).unwrap();
    assert_eq!(
        editor.breadcrumb(page).unwrap(),
        vec![Crumb::Page {
            page,
            label: "Notes".into(),
            icon: Some("📝".into())
        }]
    );
}

#[test]
fn nested_pages_show_their_ancestors_from_the_top() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let pages = chain(&mut editor, MAX_CRUMBS);
    assert_eq!(shown(&editor, pages[3]), "A / B / C / D");
    assert_eq!(shown(&editor, pages[1]), "A / B");
}

#[test]
fn long_paths_keep_the_first_and_the_last_two() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let pages = chain(&mut editor, 6);
    assert_eq!(shown(&editor, pages[4]), "A / … / D / E");
    assert_eq!(shown(&editor, pages[5]), "A / … / E / F");
}

#[test]
fn the_breadcrumb_follows_renames_moves_and_icons() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let pages = chain(&mut editor, 2);
    editor.rename(pages[0], "").unwrap();
    editor.set_icon(pages[1], Some("🌿")).unwrap();
    assert_eq!(shown(&editor, pages[1]), format!("{UNTITLED} / 🌿 B"));
}
