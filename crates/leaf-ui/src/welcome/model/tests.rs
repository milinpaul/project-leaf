//! Tests for the welcome screen's helpers.

use std::path::{MAIN_SEPARATOR_STR, Path, PathBuf};

use leaf_store::StoreError;

use super::{open_problem, opened_label, short_path, step};

/// `path` written with this system's separator (`\\` on Windows), as `short_path` shows it.
fn native(path: &str) -> String {
    path.replace('/', MAIN_SEPARATOR_STR)
}

#[test]
fn paths_show_home_as_a_tilde_and_long_ones_are_shortened() {
    let home = Path::new("/home/sam");
    assert_eq!(
        short_path(Path::new("/home/sam/Documents/Leaf/Work.leaf"), Some(home)),
        native("~/Documents/Leaf/Work.leaf")
    );
    assert_eq!(
        short_path(Path::new("/srv/notes/Team.leaf"), Some(home)),
        native("/srv/notes/Team.leaf")
    );
    assert_eq!(
        short_path(Path::new("/srv/notes/Team.leaf"), None),
        native("/srv/notes/Team.leaf")
    );
    let long =
        PathBuf::from("/home/sam/Documents/Archive/2026/Clients/Northwind Traders/Projects/Q4 Plans.leaf");
    assert_eq!(
        short_path(&long, Some(home)),
        native("~/Documents/…/Projects/Q4 Plans.leaf")
    );
}

#[test]
fn a_path_never_mixes_separators() {
    let shown = short_path(Path::new("/srv/notes/Team.leaf"), None);
    let other = if MAIN_SEPARATOR_STR == "/" { '\\' } else { '/' };
    assert!(!shown.contains(other), "{shown}");
}

#[test]
fn the_highlight_wraps_through_the_list() {
    assert_eq!(step(None, 3, true), Some(0));
    assert_eq!(step(None, 3, false), Some(2));
    assert_eq!(step(Some(2), 3, true), Some(0));
    assert_eq!(step(Some(0), 3, false), Some(2));
    assert_eq!(step(Some(1), 3, true), Some(2));
    assert_eq!(step(None, 0, true), None, "nothing to highlight");
}

#[test]
fn recent_rows_say_when_they_were_opened() {
    let now = 1_700_000_000_000;
    assert_eq!(opened_label(now, now - 5 * 60_000), "Opened 5 min ago");
    assert_eq!(opened_label(now, now), "Opened just now");
}

#[test]
fn open_problems_are_explained_in_plain_words() {
    let here = std::env::temp_dir();
    let gone = here.join("leaf-no-such-workspace.leaf");
    assert_eq!(
        open_problem("Work", &StoreError::AlreadyOpen(here.clone())),
        "“Work” is already open in another Leaf window."
    );
    assert_eq!(
        open_problem("Work", &StoreError::NotAWorkspace(gone)),
        "Leaf can't find “Work”. It may have been moved, renamed or deleted."
    );
    assert_eq!(
        open_problem("Tmp", &StoreError::NotAWorkspace(here)),
        "“Tmp” isn't a Leaf workspace."
    );
    assert_eq!(
        open_problem(
            "Work",
            &StoreError::SchemaTooNew {
                found: 9,
                supported: 1
            }
        ),
        "“Work” was saved by a newer Leaf. Update Leaf to open it."
    );
    let io = StoreError::Io(std::io::Error::other("disk on fire"));
    assert_eq!(
        open_problem("Work", &io),
        "Leaf couldn't open “Work”: file system error: disk on fire"
    );
}
