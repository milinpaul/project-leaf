//! Tests for a page's width setting (E18-S06).

use leaf_model::Workspace;

use crate::WorkspaceEditor;

const NOW: i64 = 1_767_225_600_000;

#[test]
fn each_page_keeps_its_own_width() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let wide = editor.new_page(NOW).unwrap();
    let narrow = editor.new_page(NOW).unwrap();
    assert!(
        !editor.is_full_width(wide).unwrap(),
        "pages start in the reading column"
    );

    editor.set_full_width(wide, true).unwrap();
    assert!(editor.is_full_width(wide).unwrap());
    assert!(!editor.is_full_width(narrow).unwrap());
    editor.set_full_width(wide, true).unwrap(); // already on: nothing to do
    editor.set_full_width(wide, false).unwrap();
    assert!(!editor.is_full_width(wide).unwrap());
}

#[test]
fn switching_is_not_an_edit() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.stamp_edits(NOW).unwrap();
    editor.set_full_width(page, true).unwrap();
    editor.stamp_edits(NOW + 60_000).unwrap();
    assert_eq!(
        editor.workspace().page(page).unwrap().edited_at,
        NOW,
        "the page's words didn't change, so neither does its edited time"
    );
}

#[test]
fn an_unknown_page_is_an_error() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let ghost = leaf_core::PageId::new();
    assert!(editor.is_full_width(ghost).is_err());
    assert!(editor.set_full_width(ghost, true).is_err());
}
