//! Tests for the sidebar view model and page actions.

use leaf_core::PageId;
use leaf_model::{ModelError, Workspace, WorkspaceCommand};

use super::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};

const NOW: i64 = 1_700_000_000_000;

fn labels(editor: &WorkspaceEditor) -> Vec<String> {
    editor
        .sidebar()
        .unwrap()
        .into_iter()
        .map(|item| item.label)
        .collect()
}

#[test]
fn starts_with_an_empty_sidebar_and_nothing_selected() {
    let editor = WorkspaceEditor::new(Workspace::new());
    assert_eq!(editor.sidebar().unwrap(), Vec::<SidebarItem>::new());
    assert_eq!(editor.selected(), None);
    assert_eq!(editor.selected_title().unwrap(), None);
    assert!(editor.workspace().is_empty());
}

#[test]
fn new_page_appends_selects_and_shows_untitled() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let first = editor.new_page(NOW).unwrap();
    let second = editor.new_page(NOW + 1).unwrap();

    let items = editor.sidebar().unwrap();
    assert_eq!(
        items.iter().map(|i| i.page).collect::<Vec<_>>(),
        vec![first, second],
        "new pages go last"
    );
    assert_eq!(
        items[1],
        SidebarItem {
            page: second,
            label: UNTITLED.into(),
            icon: None,
            selected: true
        }
    );
    assert!(!items[0].selected);
    assert_eq!(
        editor.selected_title().unwrap().as_deref(),
        Some(""),
        "the stored title stays empty"
    );
    assert_eq!(editor.workspace().page(second).unwrap().created_at, NOW + 1);
}

#[test]
fn renaming_the_selected_page_updates_the_sidebar() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.rename_selected("Ideas").unwrap();
    assert_eq!(labels(&editor), vec!["Ideas"]);
    assert_eq!(editor.selected_title().unwrap().as_deref(), Some("Ideas"));

    editor.rename_selected("").unwrap();
    assert_eq!(
        labels(&editor),
        vec![UNTITLED],
        "clearing the title shows Untitled again"
    );
    assert_eq!(editor.workspace().page(page).unwrap().title, "");
}

#[test]
fn renaming_to_the_same_title_or_with_nothing_selected_changes_nothing() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    editor.rename_selected("ignored").unwrap();
    assert!(editor.workspace().is_empty());

    editor.new_page(NOW).unwrap();
    editor.rename_selected("Ideas").unwrap();
    let before = editor.workspace().to_bytes().unwrap();
    editor.rename_selected("Ideas").unwrap();
    assert_eq!(
        editor.workspace().to_bytes().unwrap(),
        before,
        "no new change is recorded"
    );
}

#[test]
fn selecting_switches_pages_and_rejects_unknown_ones() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let a = editor.new_page(NOW).unwrap();
    editor.rename_selected("A").unwrap();
    let b = editor.new_page(NOW).unwrap();
    editor.rename_selected("B").unwrap();

    editor.select(a).unwrap();
    assert_eq!(editor.selected(), Some(a));
    assert_eq!(editor.selected_title().unwrap().as_deref(), Some("A"));
    editor.rename_selected("Alpha").unwrap();
    assert_eq!(
        labels(&editor),
        vec!["Alpha", "B"],
        "only the selected page is renamed"
    );
    assert_eq!(editor.workspace().page(b).unwrap().title, "B");

    let ghost = PageId::new();
    let err = editor.select(ghost).unwrap_err();
    assert!(matches!(err, EditorError::Model(ModelError::PageNotFound(p)) if p == ghost));
    assert_eq!(err.to_string(), format!("page {ghost} not found"));
    assert_eq!(
        editor.selected(),
        Some(a),
        "a failed select keeps the current page"
    );
}

#[test]
fn sidebar_shows_icons_and_hides_trashed_pages() {
    // Trash commands arrive in E01-S05, so write a trashed page with raw Loro and load it.
    let (kept, trashed) = (PageId::new(), PageId::new());
    let doc = loro::LoroDoc::new();
    let tree = doc.get_tree("pages");
    tree.enable_fractional_index(0);
    for (page, title, trashed_at) in [(kept, "Kept", None), (trashed, "Binned", Some(NOW))] {
        let meta = tree.get_meta(tree.create(None).unwrap()).unwrap();
        meta.insert("id", page.to_string()).unwrap();
        meta.insert("title", title).unwrap();
        meta.insert("created_at", NOW).unwrap();
        if let Some(at) = trashed_at {
            meta.insert("trashed_at", at).unwrap();
        }
    }
    doc.commit();
    let mut workspace = Workspace::from_bytes(&doc.export(loro::ExportMode::Snapshot).unwrap()).unwrap();
    workspace
        .apply(WorkspaceCommand::SetPageIcon {
            page: kept,
            icon: Some("🌱".into()),
        })
        .unwrap();

    let editor = WorkspaceEditor::new(workspace);
    let items = editor.sidebar().unwrap();
    assert_eq!(items.len(), 1, "trashed pages are not listed");
    assert_eq!((items[0].page, items[0].icon.as_deref()), (kept, Some("🌱")));
}
