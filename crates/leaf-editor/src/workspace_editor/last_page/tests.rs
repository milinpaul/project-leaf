//! Tests for reopening the last open page (E06-S19).

use leaf_core::PageId;
use leaf_model::Workspace;

use crate::WorkspaceEditor;

const NOW: i64 = 1_767_225_600_000;

#[test]
fn the_saved_page_opens_again() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let first = editor.new_page(NOW).unwrap();
    let second = editor.new_page(NOW).unwrap();
    let mut reopened =
        WorkspaceEditor::new(Workspace::from_bytes(&editor.workspace().to_bytes().unwrap()).unwrap());
    assert_eq!(reopened.selected(), None, "nothing is open at first");
    assert!(reopened.restore_open_page(Some(first)));
    assert_eq!(reopened.selected(), Some(first));
    assert!(reopened.restore_open_page(Some(second)));
    assert_eq!(reopened.selected(), Some(second));
}

#[test]
fn a_missing_trashed_or_unsaved_page_is_skipped() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let kept = editor.new_page(NOW).unwrap();
    let parent = editor.new_page(NOW).unwrap();
    let child = editor.new_page_in(Some(parent), NOW).unwrap();
    editor.trash(parent, NOW).unwrap();
    let mut reopened =
        WorkspaceEditor::new(Workspace::from_bytes(&editor.workspace().to_bytes().unwrap()).unwrap());
    assert!(!reopened.restore_open_page(None));
    assert!(
        !reopened.restore_open_page(Some(PageId::new())),
        "not in this workspace"
    );
    assert!(!reopened.restore_open_page(Some(parent)), "in the trash");
    assert!(
        !reopened.restore_open_page(Some(child)),
        "inside a page in the trash"
    );
    assert_eq!(reopened.selected(), None);
    assert!(reopened.restore_open_page(Some(kept)));
}
