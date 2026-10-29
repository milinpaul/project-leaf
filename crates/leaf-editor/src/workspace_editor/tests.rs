//! Tests for the sidebar view model and page actions.

use leaf_core::PageId;
use leaf_model::{ModelError, Workspace, WorkspaceCommand};

use super::{EditorError, SidebarItem, UNTITLED, WorkspaceEditor};
use crate::{BuiltinCover, PageCover};

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
            selected: true,
            depth: 0,
            has_children: false,
            expanded: false,
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

#[test]
fn each_page_gets_its_own_blocks_kept_between_opens() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    assert!(editor.open_page().unwrap().is_none(), "no page open yet");

    let first = editor.new_page(NOW).unwrap();
    let block = {
        let page = editor.open_page().unwrap().unwrap();
        let rows = page.rows().unwrap();
        assert_eq!(rows.len(), 1, "a new page starts with one empty paragraph");
        page.set_text(rows[0].id, "first page").unwrap();
        rows[0].id
    };
    let second = editor.new_page(NOW).unwrap();
    assert_eq!(
        editor.open_page().unwrap().unwrap().rows().unwrap()[0]
            .text
            .as_deref(),
        Some("")
    );

    editor.select(first).unwrap();
    let page = editor.open_page().unwrap().unwrap();
    assert_eq!(
        page.doc().plain_text(block).unwrap().as_deref(),
        Some("first page"),
        "kept between opens"
    );
    assert!(editor.page_editor(second).is_some());
    assert!(editor.page_editor(PageId::new()).is_none(), "never opened");
}

#[test]
fn pages_loaded_from_disk_are_used_and_listed_for_saving() {
    use leaf_model::{BlockKind, PageCommand, PageDoc};

    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(1).unwrap();
    let mut saved = PageDoc::new();
    saved
        .apply(PageCommand::InsertBlock {
            parent: None,
            index: 0,
            kind: BlockKind::Quote,
            text: "from disk".into(),
        })
        .unwrap();
    editor.insert_page(page, saved).unwrap();
    let rows = editor.open_page().unwrap().unwrap().rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text.as_deref(), Some("from disk"));

    // A second hand-over doesn't replace the live copy.
    editor.insert_page(page, PageDoc::new()).unwrap();
    assert_eq!(editor.page_editor(page).unwrap().doc().len(), 1);
    // An empty page from disk gets a paragraph to type in.
    let other = editor.new_page(2).unwrap();
    editor.insert_page(other, PageDoc::new()).unwrap();
    assert_eq!(editor.page_editor(other).unwrap().doc().len(), 1);

    let mut listed: Vec<PageId> = editor.loaded_pages().map(|(id, _)| id).collect();
    listed.sort();
    let mut expected = vec![page, other];
    expected.sort();
    assert_eq!(listed, expected);
    assert!(matches!(
        editor.insert_page(PageId::new(), PageDoc::new()),
        Err(EditorError::Model(ModelError::PageNotFound(_)))
    ));
}

#[test]
fn a_new_page_goes_last_even_when_trashed_pages_follow_the_last_visible_one() {
    // Positions count trashed pages, so appending at the visible count lands among them.
    let mut workspace = Workspace::new();
    let create = |workspace: &mut Workspace, index| match workspace
        .apply(WorkspaceCommand::CreatePage {
            parent: None,
            index,
            title: String::new(),
            created_at: NOW,
        })
        .unwrap()
    {
        leaf_model::Outcome::PageCreated(page) => page,
        other => panic!("unexpected {other:?}"),
    };
    let a = create(&mut workspace, 0);
    let binned = create(&mut workspace, 1);
    let b = create(&mut workspace, 2);
    workspace
        .apply(WorkspaceCommand::TrashPage {
            page: binned,
            at: NOW,
        })
        .unwrap();

    let mut editor = WorkspaceEditor::new(workspace);
    let new = editor.new_page(NOW).unwrap();
    let order: Vec<PageId> = editor.sidebar().unwrap().into_iter().map(|i| i.page).collect();
    assert_eq!(order, vec![a, b, new]);
}

#[test]
fn setting_and_removing_a_page_icon() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    assert_eq!(editor.selected_icon().unwrap(), None, "no page open");
    let page = editor.new_page(NOW).unwrap();
    assert_eq!(editor.selected_icon().unwrap(), None);
    editor.set_icon(page, Some("🌱")).unwrap();
    assert_eq!(editor.selected_icon().unwrap().as_deref(), Some("🌱"));
    assert_eq!(
        editor.sidebar().unwrap()[0].icon.as_deref(),
        Some("🌱"),
        "the sidebar shows it"
    );

    let before = editor.workspace().to_bytes().unwrap();
    editor.set_icon(page, Some("🌱")).unwrap();
    assert_eq!(
        editor.workspace().to_bytes().unwrap(),
        before,
        "the same icon records nothing"
    );

    editor.set_icon(page, None).unwrap();
    assert_eq!(editor.selected_icon().unwrap(), None);
    assert!(editor.set_icon(PageId::new(), Some("x")).is_err());
}

#[test]
fn setting_and_removing_a_page_cover() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    assert_eq!(editor.selected_cover().unwrap(), None, "no page open");
    let page = editor.new_page(NOW).unwrap();
    assert_eq!(editor.selected_cover().unwrap(), None);

    editor.set_cover(page, Some(BuiltinCover::Dawn)).unwrap();
    assert_eq!(
        editor.selected_cover().unwrap(),
        Some(PageCover::Builtin(BuiltinCover::Dawn))
    );
    assert_eq!(
        editor.workspace().page(page).unwrap().cover.as_deref(),
        Some("builtin:dawn")
    );

    let before = editor.workspace().to_bytes().unwrap();
    editor.set_cover(page, Some(BuiltinCover::Dawn)).unwrap();
    assert_eq!(
        editor.workspace().to_bytes().unwrap(),
        before,
        "the same cover records nothing"
    );

    editor.set_cover(page, None).unwrap();
    assert_eq!(editor.selected_cover().unwrap(), None);
    assert!(editor.set_cover(PageId::new(), Some(BuiltinCover::Dawn)).is_err());
}

#[test]
fn a_cover_from_a_newer_version_reads_as_unknown_and_can_be_removed() {
    let mut workspace = Workspace::new();
    let page = match workspace
        .apply(WorkspaceCommand::CreatePage {
            parent: None,
            index: 0,
            title: "From the future".into(),
            created_at: NOW,
        })
        .unwrap()
    {
        leaf_model::Outcome::PageCreated(page) => page,
        other => panic!("expected a new page, got {other:?}"),
    };
    workspace
        .apply(WorkspaceCommand::SetPageCover {
            page,
            cover: Some("builtin:aurora".into()),
        })
        .unwrap();
    let mut editor = WorkspaceEditor::new(workspace);
    editor.select(page).unwrap();

    assert_eq!(
        editor.selected_cover().unwrap(),
        Some(PageCover::Unknown("builtin:aurora".into()))
    );
    editor.set_cover(page, None).unwrap();
    assert_eq!(editor.selected_cover().unwrap(), None);
}
