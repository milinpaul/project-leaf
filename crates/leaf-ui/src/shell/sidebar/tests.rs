//! The workspace menu ticks the open workspace however its path was written.

use super::same_folder;

#[test]
fn the_open_workspace_is_ticked_through_a_link() {
    let dir = std::env::temp_dir().join(format!("leaf-ui-sidebar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let real = dir.join("Work.leaf");
    std::fs::create_dir_all(&real).unwrap();
    std::fs::create_dir_all(dir.join("x")).unwrap();
    // The recent list stores the resolved path; the window may hold one written another way.
    let stored = std::fs::canonicalize(&real).unwrap();
    assert!(same_folder(&dir.join("x").join("..").join("Work.leaf"), &stored));
    #[cfg(unix)]
    {
        let link = dir.join("Link.leaf");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(
            same_folder(&link, &stored),
            "opened through a link, as macOS's /var is"
        );
    }
    assert!(!same_folder(&dir.join("Other.leaf"), &stored));
    let _ = std::fs::remove_dir_all(dir);
}
