//! Nothing stored in, or typed for, a workspace can make Leaf touch a path outside the workspace
//! folder (E08-S13, ADR-074).
#![allow(clippy::panic, reason = "test helpers fail a test by panicking")]

mod common;

use std::path::{Component, Path};

use common::TempDir;
use leaf_core::AssetId;
use leaf_store::{NameProblem, Workspace, new_workspace_path};
use proptest::prelude::*;

/// The path is exactly one level below `location`, with a plain last part.
fn directly_inside(location: &Path, path: &Path) -> bool {
    path.parent() == Some(location)
        && path
            .file_name()
            .is_some_and(|name| matches!(Path::new(name).components().next(), Some(Component::Normal(_))))
}

#[test]
fn names_that_could_escape_the_chosen_folder_are_refused() {
    let dir = TempDir::new();
    for bad in [
        "..",
        ".",
        "../outside",
        "a/../../b",
        "/etc",
        "C:\\Windows",
        "C:",
        "\\\\server\\share",
        "//server/share",
        "line\nbreak",
        "nul\0byte",
        "tab\there",
    ] {
        assert_eq!(
            new_workspace_path(&dir.0, bad),
            Err(NameProblem::Unsafe),
            "{bad:?}"
        );
    }
}

#[test]
fn windows_device_names_are_refused_whatever_their_case_or_ending() {
    let dir = TempDir::new();
    for bad in [
        "CON",
        "con",
        "Prn",
        "AUX",
        "NUL",
        "nul.leaf",
        "COM1",
        "com9",
        "LPT1",
        "lpt9",
        "COM0",
        "COM¹",
        "LPT³",
        "CON.notes",
        "aux .leaf",
        "NUL ",
    ] {
        assert_eq!(
            new_workspace_path(&dir.0, bad),
            Err(NameProblem::Reserved(bad.trim().to_owned())),
            "{bad:?}"
        );
    }
    assert_eq!(
        NameProblem::Reserved("CON".to_owned()).to_string(),
        "“CON” is a name Windows keeps for itself. Pick another name."
    );
}

#[test]
fn names_that_only_look_like_device_names_are_fine() {
    let dir = TempDir::new();
    for good in [
        "Console",
        "CONTRACTS",
        "nul points",
        "COM10",
        "LPT",
        "my CON",
        "Auxiliary",
    ] {
        assert_eq!(
            new_workspace_path(&dir.0, good),
            Ok(dir.0.join(format!("{good}.leaf"))),
            "{good:?}"
        );
    }
}

proptest! {
    #[test]
    fn any_accepted_name_stays_directly_inside_the_chosen_folder(name in any::<String>()) {
        let dir = TempDir::new();
        if let Ok(path) = new_workspace_path(&dir.0, &name) {
            prop_assert!(directly_inside(&dir.0, &path), "{name:?} became {}", path.display());
            let folder = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            prop_assert!(
                Path::new(folder).extension().is_some_and(|ext| ext.eq_ignore_ascii_case("leaf")),
                "{folder:?}"
            );
        }
    }

    #[test]
    fn an_image_file_always_lives_directly_in_assets(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let dir = TempDir::new();
        let workspace = Workspace::create(&dir.0.join("w.leaf"));
        prop_assert!(workspace.is_ok());
        let Ok(workspace) = workspace else { return Ok(()) };
        let assets = dir.0.join("w.leaf").join("assets");
        let path = workspace.asset_path(AssetId::of(&bytes));
        prop_assert!(directly_inside(&assets, &path), "{}", path.display());
    }
}

#[test]
fn asset_ids_never_parse_from_path_like_text() {
    for bad in ["..", "../x", "/etc/passwd", "a/b", "C:\\x", ""] {
        assert!(bad.parse::<AssetId>().is_err(), "{bad:?}");
    }
}

#[cfg(unix)]
mod links {
    use std::fmt::Debug;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    use leaf_store::{StoreError, Workspace};

    use super::common::TempDir;

    // The workspace's own names (`leaf_store::workspace`), spelled out as the folder shows them.
    const ASSETS: &str = "assets";
    const SEARCH: &str = "search";
    const DATABASE: &str = "workspace.sqlite";
    const LOCK: &str = "leaf.lock";

    /// The value, or a test failure saying what couldn't be set up.
    fn ok<T, E: Debug>(result: Result<T, E>, what: &str) -> T {
        match result {
            Ok(value) => value,
            Err(err) => panic!("{what}: {err:?}"),
        }
    }

    /// A closed workspace at `dir/w.leaf` and an outside folder `dir/outside` with one file in it.
    fn workspace_and_outside(dir: &TempDir) -> (PathBuf, PathBuf) {
        let root = dir.0.join("w.leaf");
        drop(ok(Workspace::create(&root), "create the workspace"));
        let outside = dir.0.join("outside");
        ok(fs::create_dir(&outside), "create the outside folder");
        ok(
            fs::write(outside.join("keep.txt"), "untouched"),
            "write the outside file",
        );
        (root, outside)
    }

    /// Everything in `folder`, sorted, so a test can see that nothing was added.
    fn listing(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = ok(fs::read_dir(folder), "list the folder")
            .map(|entry| {
                ok(entry, "read an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn assert_refused(root: &Path, link: &Path, outside: &Path) {
        match Workspace::open(root) {
            Err(StoreError::LinkInWorkspace(found)) => assert_eq!(&found, link),
            other => panic!("expected LinkInWorkspace({}), got {other:?}", link.display()),
        }
        assert_eq!(
            listing(outside),
            vec!["keep.txt".to_owned()],
            "nothing written outside"
        );
        assert!(
            ok(fs::symlink_metadata(link), "look at the link")
                .file_type()
                .is_symlink(),
            "the link is left as it was"
        );
    }

    #[test]
    fn a_linked_assets_folder_is_refused() {
        let dir = TempDir::new();
        let (root, outside) = workspace_and_outside(&dir);
        let link = root.join(ASSETS);
        fs::remove_dir_all(&link).unwrap();
        symlink(&outside, &link).unwrap();
        assert_refused(&root, &link, &outside);
    }

    #[test]
    fn a_linked_search_folder_is_refused() {
        let dir = TempDir::new();
        let (root, outside) = workspace_and_outside(&dir);
        let link = root.join(SEARCH);
        fs::remove_dir_all(&link).unwrap();
        symlink(&outside, &link).unwrap();
        assert_refused(&root, &link, &outside);
    }

    #[test]
    fn a_linked_file_inside_assets_or_search_is_refused() {
        for folder in [ASSETS, SEARCH] {
            let dir = TempDir::new();
            let (root, outside) = workspace_and_outside(&dir);
            let link = root.join(folder).join("planted");
            symlink(outside.join("keep.txt"), &link).unwrap();
            assert_refused(&root, &link, &outside);
        }
    }

    #[test]
    fn linked_files_at_the_top_of_the_workspace_are_refused() {
        for name in [LOCK, "workspace.sqlite-wal", "view.json.tmp"] {
            let dir = TempDir::new();
            let (root, outside) = workspace_and_outside(&dir);
            let link = root.join(name);
            let _ = fs::remove_file(&link);
            symlink(outside.join("new-file"), &link).unwrap();
            assert_refused(&root, &link, &outside);
        }
    }

    #[test]
    fn a_linked_database_is_refused_before_anything_is_written() {
        let dir = TempDir::new();
        let (root, outside) = workspace_and_outside(&dir);
        let database = root.join(DATABASE);
        fs::rename(&database, outside.join("moved.sqlite")).unwrap();
        symlink(outside.join("moved.sqlite"), &database).unwrap();
        match Workspace::open(&root) {
            Err(StoreError::LinkInWorkspace(found)) => assert_eq!(found, database),
            other => panic!("expected LinkInWorkspace, got {other:?}"),
        }
        assert_eq!(
            listing(&outside),
            vec!["keep.txt".to_owned(), "moved.sqlite".to_owned()]
        );
    }

    #[test]
    fn the_workspace_folder_itself_may_be_a_link() {
        let dir = TempDir::new();
        let (root, _) = workspace_and_outside(&dir);
        let shortcut = dir.0.join("shortcut.leaf");
        symlink(&root, &shortcut).unwrap();
        assert!(Workspace::open(&shortcut).is_ok());
    }

    #[test]
    fn the_error_names_the_link() {
        let err = StoreError::LinkInWorkspace(PathBuf::from("/w.leaf/assets"));
        assert_eq!(
            err.to_string(),
            "/w.leaf/assets is a link to another place; Leaf only opens workspaces without links"
        );
    }
}
