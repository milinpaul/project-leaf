//! Tests for importing images into a workspace.

use std::fs;
use std::path::PathBuf;

use leaf_core::AssetId;

use crate::{StoreError, Workspace};

const NOW: i64 = 1_700_000_000_000;

/// A tiny but well-formed-looking PNG: the signature, then some bytes that make it unique.
fn png(tag: &str) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(tag.as_bytes());
    bytes
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("leaf-store-assets-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn files_in(folder: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn importing_stores_the_file_by_its_hash_and_records_type_and_size() {
    let dir = temp_dir("import");
    let mut workspace = Workspace::create(&dir).unwrap();
    let bytes = png("sunset");
    let asset = workspace.import_asset(&bytes, NOW).unwrap();
    assert_eq!(asset.id, AssetId::of(&bytes));
    assert_eq!(asset.mime, "image/png");
    assert_eq!(asset.bytes, u64::try_from(bytes.len()).unwrap());
    assert_eq!(asset.created_at, NOW);
    let path = workspace.asset_path(asset.id);
    assert_eq!(path, dir.join("assets").join(asset.id.to_string()));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(workspace.asset(asset.id).unwrap(), Some(asset));
    assert_eq!(
        files_in(&dir.join("assets")),
        vec![AssetId::of(&bytes).to_string()]
    );
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_same_image_is_stored_once() {
    let dir = temp_dir("dedupe");
    let mut workspace = Workspace::create(&dir).unwrap();
    let first = workspace.import_asset(&png("cat"), NOW).unwrap();
    let again = workspace.import_asset(&png("cat"), NOW + 5_000).unwrap();
    assert_eq!(
        again, first,
        "the first record is kept, including when it was imported"
    );
    let other = workspace.import_asset(&png("dog"), NOW + 9_000).unwrap();
    assert_ne!(other.id, first.id);
    assert_eq!(
        files_in(&dir.join("assets")).len(),
        2,
        "one file per distinct image"
    );
    let rows: i64 = workspace
        .store()
        .conn
        .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, 2);
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_file_that_is_not_an_image_is_refused_and_nothing_is_stored() {
    let dir = temp_dir("refuse");
    let mut workspace = Workspace::create(&dir).unwrap();
    let err = workspace
        .import_asset(b"%PDF-1.7 not a picture", NOW)
        .unwrap_err();
    assert!(matches!(err, StoreError::NotAnImage), "{err:?}");
    assert_eq!(files_in(&dir.join("assets")), Vec::<String>::new());
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_damaged_copy_on_disk_is_repaired_by_the_next_import() {
    let dir = temp_dir("repair");
    let mut workspace = Workspace::create(&dir).unwrap();
    let bytes = png("map");
    let asset = workspace.import_asset(&bytes, NOW).unwrap();
    let path = workspace.asset_path(asset.id);
    fs::write(&path, b"truncated").unwrap();
    workspace.import_asset(&bytes, NOW + 1).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        bytes,
        "rewritten from the imported bytes"
    );
    assert!(
        !files_in(&dir.join("assets"))
            .iter()
            .any(|name| std::path::Path::new(name)
                .extension()
                .is_some_and(|ext| ext == "tmp")),
        "no temporary file is left behind"
    );
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn importing_a_file_reads_it_from_disk() {
    let dir = temp_dir("from-file");
    let mut workspace = Workspace::create(&dir.join("w.leaf")).unwrap();
    let source = dir.join("photo.gif");
    fs::write(&source, b"GIF89a\x01\0\x01\0 pixels").unwrap();
    let asset = workspace.import_asset_file(&source, NOW).unwrap();
    assert_eq!(asset.mime, "image/gif");
    assert_eq!(
        fs::read(workspace.asset_path(asset.id)).unwrap(),
        fs::read(&source).unwrap()
    );
    let missing = workspace
        .import_asset_file(&dir.join("nope.png"), NOW)
        .unwrap_err();
    assert!(matches!(missing, StoreError::Io(_)), "{missing:?}");
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_unknown_asset_has_no_record() {
    let dir = temp_dir("unknown");
    let workspace = Workspace::create(&dir).unwrap();
    assert_eq!(workspace.asset(AssetId::of(b"never imported")).unwrap(), None);
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn records_survive_reopening_the_workspace() {
    let dir = temp_dir("reopen");
    let mut workspace = Workspace::create(&dir).unwrap();
    let asset = workspace.import_asset(&png("kept"), NOW).unwrap();
    drop(workspace);
    let workspace = Workspace::open(&dir).unwrap();
    assert_eq!(workspace.asset(asset.id).unwrap(), Some(asset));
    drop(workspace);
    let _ = fs::remove_dir_all(dir);
}
