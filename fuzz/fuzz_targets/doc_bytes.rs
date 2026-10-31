//! Saved document bytes, as a damaged or crafted workspace could hold them, loaded the ways the store
//! loads them: a snapshot plus updates, the recovery path that skips bad updates, and changes merged into
//! a fresh document. The first byte picks the document type (page, workspace tree or database); the rest
//! is cut at `leaf_fuzz::PART` into snapshot and updates.
#![no_main]

use leaf_fuzz::{parts, walk_database, walk_page, walk_workspace};
use leaf_model::{Database, PageDoc, Workspace};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    leaf_fuzz::allow_caught_panics();
    let Some((&kind, rest)) = data.split_first() else {
        return;
    };
    let pieces = parts(rest);
    let (snapshot, updates) = (
        pieces[0],
        pieces[1..].iter().map(|u| u.to_vec()).collect::<Vec<_>>(),
    );
    match kind % 3 {
        0 => {
            if let Ok(page) = PageDoc::from_saved(snapshot, &updates) {
                walk_page(&page);
            }
            if let Ok((page, _)) = PageDoc::recover(snapshot, &updates) {
                walk_page(&page);
            }
            let mut page = PageDoc::new();
            if page.import_changes(rest).is_ok() {
                walk_page(&page);
            }
        }
        1 => {
            if let Ok(workspace) = Workspace::from_saved(snapshot, &updates) {
                walk_workspace(&workspace);
            }
            if let Ok((workspace, _)) = Workspace::recover(snapshot, &updates) {
                walk_workspace(&workspace);
            }
            let mut workspace = Workspace::default();
            if workspace.import_changes(rest).is_ok() {
                walk_workspace(&workspace);
            }
        }
        _ => {
            if let Ok(database) = Database::from_saved(snapshot, &updates) {
                walk_database(&database);
            }
            if let Ok((database, _)) = Database::recover(snapshot, &updates) {
                walk_database(&database);
            }
        }
    }
});
