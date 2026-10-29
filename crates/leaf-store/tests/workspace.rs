//! Workspace folders: create, open, and one app at a time (E03-S02).

mod common;

use std::fs;

use common::TempDir;
use leaf_store::{SCHEMA_VERSION, StoreError, Workspace};

#[test]
fn creating_a_workspace_lays_out_the_folder() {
    let dir = TempDir::new();
    let path = dir.0.join("My Notes.leaf");
    let workspace = Workspace::create(&path).unwrap();
    assert_eq!(workspace.root(), path);
    assert_eq!(workspace.store().schema_version().unwrap(), SCHEMA_VERSION);
    assert!(path.join("workspace.sqlite").is_file());
    assert!(path.join("assets").is_dir());
    assert!(path.join("search").is_dir());
    assert!(path.join("leaf.lock").is_file());
}

#[test]
fn creating_in_an_empty_folder_is_fine_but_never_over_existing_files() {
    let dir = TempDir::new();
    let empty = dir.0.join("empty.leaf");
    fs::create_dir(&empty).unwrap();
    drop(Workspace::create(&empty).unwrap());

    assert!(matches!(
        Workspace::create(&empty),
        Err(StoreError::AlreadyExists(p)) if p == empty
    ));
    let file = dir.0.join("notes.txt");
    fs::write(&file, "mine").unwrap();
    assert!(matches!(
        Workspace::create(&file),
        Err(StoreError::AlreadyExists(_))
    ));
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine", "left untouched");
}

#[test]
fn a_created_workspace_opens_again_with_its_data() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    drop(Workspace::create(&path).unwrap());
    let workspace = Workspace::open(&path).unwrap();
    assert_eq!(workspace.store().schema_version().unwrap(), SCHEMA_VERSION);
}

#[test]
fn opening_something_that_is_not_a_workspace_is_refused() {
    let dir = TempDir::new();
    let missing = dir.0.join("missing.leaf");
    let plain = dir.0.join("plain");
    fs::create_dir(&plain).unwrap();
    let file = dir.0.join("file.leaf");
    fs::write(&file, "x").unwrap();
    for path in [&missing, &plain, &file] {
        assert!(
            matches!(Workspace::open(path), Err(StoreError::NotAWorkspace(p)) if &p == path),
            "{} must be refused",
            path.display()
        );
    }
    assert!(!missing.exists(), "opening never creates anything");
    assert_eq!(fs::read_dir(&plain).unwrap().count(), 0);
}

#[test]
fn a_corrupt_database_is_an_error_not_a_fresh_start() {
    let dir = TempDir::new();
    let path = dir.0.join("bad.leaf");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("workspace.sqlite"), vec![7_u8; 4096]).unwrap();
    assert!(matches!(Workspace::open(&path), Err(StoreError::Sqlite(_))));
    assert_eq!(fs::read(path.join("workspace.sqlite")).unwrap(), vec![7_u8; 4096]);
}

#[test]
fn only_one_app_at_a_time_and_the_lock_goes_with_it() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    let first = Workspace::create(&path).unwrap();
    assert!(matches!(
        Workspace::open(&path),
        Err(StoreError::AlreadyOpen(p)) if p == path
    ));
    drop(first);
    assert!(
        path.join("leaf.lock").exists(),
        "the file stays; only the OS lock is released"
    );
    let _second = Workspace::open(&path).unwrap();
}

#[test]
fn missing_subfolders_are_recreated_on_open() {
    let dir = TempDir::new();
    let path = dir.0.join("w.leaf");
    drop(Workspace::create(&path).unwrap());
    fs::remove_dir(path.join("assets")).unwrap();
    fs::remove_dir(path.join("search")).unwrap();
    let _workspace = Workspace::open(&path).unwrap();
    assert!(path.join("assets").is_dir() && path.join("search").is_dir());
}

#[test]
fn errors_name_the_folder() {
    let path = std::path::PathBuf::from("/tmp/x.leaf");
    assert_eq!(
        StoreError::AlreadyOpen(path.clone()).to_string(),
        "/tmp/x.leaf is already open in another Leaf window"
    );
    assert_eq!(
        StoreError::NotAWorkspace(path.clone()).to_string(),
        "/tmp/x.leaf is not a Leaf workspace"
    );
    assert_eq!(
        StoreError::AlreadyExists(path).to_string(),
        "/tmp/x.leaf already exists and is not an empty folder"
    );
}

#[test]
fn open_or_create_makes_a_missing_workspace_and_reopens_an_existing_one() {
    let dir = TempDir::new();
    let path = dir.0.join("Leaf").join("My Workspace.leaf");
    drop(Workspace::open_or_create(&path).unwrap());
    assert!(path.join("workspace.sqlite").is_file(), "created, parents too");
    drop(Workspace::open_or_create(&path).unwrap());

    let empty = dir.0.join("empty");
    fs::create_dir(&empty).unwrap();
    drop(Workspace::open_or_create(&empty).unwrap());

    let busy = dir.0.join("busy");
    fs::create_dir(&busy).unwrap();
    fs::write(busy.join("notes.txt"), "mine").unwrap();
    assert!(matches!(
        Workspace::open_or_create(&busy),
        Err(StoreError::AlreadyExists(_))
    ));
    let first = Workspace::open_or_create(&path).unwrap();
    assert!(matches!(
        Workspace::open_or_create(&path),
        Err(StoreError::AlreadyOpen(_))
    ));
    drop(first);
}

#[test]
fn a_reader_sees_what_the_workspace_saves() {
    let dir = TempDir::new();
    let mut ws = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let reader = ws.reader().unwrap();
    assert!(reader.saved_versions().unwrap().is_empty());

    ws.store_mut()
        .save(leaf_store::DocKey::Workspace, &leaf_model::Workspace::new(), 7)
        .unwrap();
    let versions = reader.saved_versions().unwrap();
    assert_eq!(versions[&leaf_store::DocKey::Workspace].saved_at, 7);
    assert!(
        reader
            .load::<leaf_model::Workspace>(leaf_store::DocKey::Workspace)
            .unwrap()
            .is_some()
    );
}
