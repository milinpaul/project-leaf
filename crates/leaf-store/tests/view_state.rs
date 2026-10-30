//! A workspace's view state (`view.json`): expanded sidebar pages, kept across launches (E06-S03).

mod common;

use std::fs;

use common::TempDir;
use leaf_core::PageId;
use leaf_store::{VIEW_STATE, ViewState, Workspace};

#[test]
fn expanded_pages_round_trip_through_the_workspace_folder() {
    let dir = TempDir::new();
    let workspace = Workspace::create(&dir.0.join("W.leaf")).unwrap();
    let path = workspace.view_state_path();
    assert_eq!(path, workspace.root().join(VIEW_STATE));
    assert_eq!(
        ViewState::load(&path),
        ViewState::default(),
        "a new workspace has none"
    );

    let pages = [PageId::new(), PageId::new()];
    let mut state = ViewState::default();
    state.set_expanded(&pages);
    state.save(&path).unwrap();
    assert_eq!(ViewState::load(&path).expanded(), pages);
    assert!(
        !path.with_extension("json.tmp").exists(),
        "no temporary file is left behind"
    );
}

#[test]
fn bad_entries_are_skipped_and_unknown_fields_kept() {
    let dir = TempDir::new();
    let path = dir.0.join(VIEW_STATE);
    let page = PageId::new();
    fs::write(
        &path,
        format!(r#"{{"expanded": ["{page}", "not-an-id"], "zoom": 2}}"#),
    )
    .unwrap();
    let mut state = ViewState::load(&path);
    assert_eq!(state.expanded(), vec![page]);

    state.set_expanded(&[]);
    state.save(&path).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        saved,
        serde_json::json!({ "zoom": 2 }),
        "a newer Leaf's field survives"
    );
}

#[test]
fn a_damaged_file_gives_defaults_and_is_moved_aside() {
    let dir = TempDir::new();
    let path = dir.0.join(VIEW_STATE);
    fs::write(&path, "{ not json").unwrap();
    assert_eq!(ViewState::load(&path), ViewState::default());
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(dir.0.join("view.json.bad")).unwrap(),
        "{ not json"
    );

    fs::create_dir(&path).unwrap(); // unreadable as a file
    assert_eq!(ViewState::load(&path), ViewState::default());
}

#[test]
fn the_open_page_round_trips_and_a_bad_or_missing_one_reads_as_none() {
    let dir = TempDir::new();
    let path = dir.0.join(VIEW_STATE);
    let page = PageId::new();
    let mut state = ViewState::default();
    assert_eq!(state.open_page(), None);
    state.set_open_page(Some(page));
    state.save(&path).unwrap();
    assert_eq!(ViewState::load(&path).open_page(), Some(page));

    // An older view.json without it, and a damaged entry, both just mean no page.
    fs::write(&path, r#"{"expanded": []}"#).unwrap();
    assert_eq!(ViewState::load(&path).open_page(), None);
    fs::write(&path, r#"{"open_page": "not-an-id"}"#).unwrap();
    assert_eq!(ViewState::load(&path).open_page(), None);

    state.set_open_page(None);
    state.save(&path).unwrap();
    assert!(
        !fs::read_to_string(&path).unwrap().contains("open_page"),
        "none is left out"
    );
}
