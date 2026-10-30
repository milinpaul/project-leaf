//! Tests for the favourites view model (E18-S02).

use leaf_model::{ModelError, Workspace};

use crate::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};

const NOW: i64 = 1_700_000_000_000;

#[test]
fn favourites_are_flat_rows_in_starred_order() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let parent = editor.new_page(NOW).unwrap();
    let child = editor.new_page_in(Some(parent), NOW).unwrap();
    let plain = editor.new_page(NOW).unwrap();
    editor.rename(parent, "Projects").unwrap();
    editor.set_icon(parent, Some("📁")).unwrap();
    editor.select(plain).unwrap();
    assert_eq!(editor.favourites().unwrap(), Vec::<SidebarItem>::new());

    for page in [plain, child, parent] {
        editor.set_favourite(page, true).unwrap();
    }
    assert_eq!(
        editor.favourites().unwrap(),
        vec![
            SidebarItem {
                page: plain,
                label: UNTITLED.into(),
                icon: None,
                selected: true,
                depth: 0,
                has_children: false,
                expanded: false,
            },
            SidebarItem {
                page: child,
                label: UNTITLED.into(),
                icon: None,
                selected: false,
                depth: 0,
                has_children: false,
                expanded: false,
            },
            SidebarItem {
                page: parent,
                label: "Projects".into(),
                icon: Some("📁".into()),
                selected: false,
                depth: 0,
                has_children: true,
                expanded: false,
            },
        ]
    );
}

#[test]
fn starring_and_unstarring_report_through_is_favourite() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    assert!(!editor.is_favourite(page));
    editor.set_favourite(page, true).unwrap();
    editor.set_favourite(page, true).unwrap();
    assert!(editor.is_favourite(page));
    assert_eq!(editor.favourites().unwrap().len(), 1);
    editor.set_favourite(page, false).unwrap();
    editor.set_favourite(page, false).unwrap();
    assert!(!editor.is_favourite(page));
}

#[test]
fn the_trash_hides_favourites_until_restored() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.set_favourite(page, true).unwrap();
    editor.trash(page, NOW).unwrap();
    assert_eq!(editor.favourites().unwrap(), Vec::<SidebarItem>::new());
    assert!(matches!(
        editor.set_favourite(page, true),
        Err(EditorError::Model(ModelError::PageInTrash(p))) if p == page
    ));
    editor.restore(page).unwrap();
    assert_eq!(editor.favourites().unwrap()[0].page, page);

    editor.trash(page, NOW).unwrap();
    editor.delete_forever(page).unwrap();
    assert_eq!(editor.favourites().unwrap(), Vec::<SidebarItem>::new());
}
