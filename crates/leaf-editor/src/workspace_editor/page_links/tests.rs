//! Tests for what the workspace tells page links.

use leaf_core::PageId;
use leaf_model::Workspace;

use super::super::WorkspaceEditor;
use crate::{LinkState, LinkTarget};

const NOW: i64 = 1_700_000_000_000;

#[test]
fn a_link_shows_the_page_as_it_is_now() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let page = editor.new_page(NOW).unwrap();
    editor.rename(page, "Trip to Japan").unwrap();
    editor.set_icon(page, Some("🗾")).unwrap();
    assert_eq!(
        editor.link_target(page),
        LinkTarget {
            title: "Trip to Japan".into(),
            icon: Some("🗾".into()),
            state: LinkState::Live,
        }
    );
    editor.rename(page, "Japan").unwrap();
    assert_eq!(editor.link_target(page).title, "Japan");

    editor.trash(page, NOW).unwrap();
    assert_eq!(editor.link_target(page).state, LinkState::Trashed);
    assert_eq!(editor.link_target(PageId::new()), LinkTarget::missing());
}

#[test]
fn only_a_title_one_page_has_is_linkable() {
    let mut editor = WorkspaceEditor::new(Workspace::new());
    let (japan, a, b, gone) = (
        editor.new_page(NOW).unwrap(),
        editor.new_page(NOW).unwrap(),
        editor.new_page(NOW).unwrap(),
        editor.new_page(NOW).unwrap(),
    );
    editor.rename(japan, " Japan ").unwrap();
    editor.rename(a, "Notes").unwrap();
    editor.rename(b, "Notes").unwrap();
    editor.rename(gone, "Old").unwrap();
    editor.trash(gone, NOW).unwrap();
    let titles = editor.titles_for_links().unwrap();
    assert_eq!(titles.get("Japan"), Some(&japan));
    assert_eq!(titles.get("Notes"), None, "two pages share it");
    assert_eq!(titles.get("Old"), None, "in the trash");
    assert_eq!(titles.len(), 1, "untitled pages aren't linkable by title");
}
