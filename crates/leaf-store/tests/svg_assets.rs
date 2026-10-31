//! A stored SVG is drawn only when it names nothing outside itself (E08-S12, ADR 0076).

mod common;

use std::fs;

use common::TempDir;
use leaf_store::{SvgProblem, Workspace};

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

#[test]
fn a_raster_picture_is_always_drawable() {
    let dir = TempDir::new();
    let mut workspace = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let asset = workspace.import_asset(PNG_HEADER, 0).unwrap();
    assert_eq!(
        workspace.drawable_asset(asset.id),
        Ok(workspace.asset_path(asset.id))
    );
}

#[test]
fn an_svg_naming_an_outside_file_is_kept_but_not_drawable() {
    let dir = TempDir::new();
    let mut workspace = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let bytes =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><image href="/etc/hosts"/></svg>"#;
    let asset = workspace.import_asset(bytes, 0).unwrap();
    assert_eq!(
        workspace.drawable_asset(asset.id),
        Err(SvgProblem::OutsideReference("/etc/hosts".to_owned()))
    );
    assert_eq!(
        fs::read(workspace.asset_path(asset.id)).unwrap(),
        bytes,
        "the file is kept as it was"
    );
}

#[test]
fn a_self_contained_svg_is_drawable() {
    let dir = TempDir::new();
    let mut workspace = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let bytes =
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect fill="url(#g)"/></svg>"#;
    let asset = workspace.import_asset(bytes, 0).unwrap();
    assert_eq!(
        workspace.drawable_asset(asset.id),
        Ok(workspace.asset_path(asset.id))
    );
}

#[test]
fn a_planted_file_that_isnt_an_image_is_checked_as_an_svg() {
    // GPUI reads anything it doesn't recognise as an SVG, so a file put in assets/ by hand is checked.
    let dir = TempDir::new();
    let workspace = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let bytes = b"not an image at all";
    let id = leaf_core::AssetId::of(bytes);
    fs::write(workspace.asset_path(id), bytes).unwrap();
    assert_eq!(workspace.drawable_asset(id), Err(SvgProblem::Unreadable));
}

#[test]
fn a_missing_file_is_left_to_the_ui() {
    let dir = TempDir::new();
    let workspace = Workspace::create(&dir.0.join("w.leaf")).unwrap();
    let id = leaf_core::AssetId::of(b"never stored");
    assert_eq!(workspace.drawable_asset(id), Ok(workspace.asset_path(id)));
}
