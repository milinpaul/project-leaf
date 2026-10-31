//! A workspace database file, as a damaged disk or a crafted download could leave it, opened the way the
//! app opens a workspace: the folder is opened and migrated, then the page tree, every page and every
//! database it names are loaded (normally and through recovery), and `view.json` is read.
//!
//! The same bytes are also tried as `view.json`, which the app reads on every open.
#![no_main]

use leaf_fuzz::{scratch, walk_database, walk_page, walk_workspace};
use leaf_model::{Database, PageDoc, Workspace};
use leaf_store::{DocKey, ViewState};
use libfuzzer_sys::fuzz_target;

/// The database file inside a workspace folder (`leaf_store`'s own name for it is private).
const DATABASE: &str = "workspace.sqlite";
/// Enough pages to reach every kind of document in a seed, without one input taking seconds.
const MAX_PAGES: usize = 64;

fuzz_target!(|data: &[u8]| {
    leaf_fuzz::allow_caught_panics();
    let folder = scratch("workspace.leaf");
    if std::fs::write(folder.join(DATABASE), data).is_err()
        || std::fs::write(folder.join(leaf_store::VIEW_STATE), data).is_err()
    {
        return;
    }
    let view = ViewState::load(&folder.join(leaf_store::VIEW_STATE));
    let _ = (view.expanded(), view.open_page());
    let Ok(open) = leaf_store::Workspace::open(&folder) else {
        return;
    };
    let store = open.store();
    let tree = match store.load::<Workspace>(DocKey::Workspace) {
        Ok(tree) => tree,
        Err(_) => store
            .recover::<Workspace>(DocKey::Workspace)
            .ok()
            .flatten()
            .map(|found| found.doc),
    };
    let Some(tree) = tree else {
        return;
    };
    let (pages, databases) = walk_workspace(&tree);
    for page in pages.into_iter().take(MAX_PAGES) {
        if let Ok(Some(doc)) = store.load::<PageDoc>(DocKey::Page(page)) {
            walk_page(&doc);
        } else if let Ok(Some(found)) = store.recover::<PageDoc>(DocKey::Page(page)) {
            walk_page(&found.doc);
        }
    }
    for database in databases.into_iter().take(MAX_PAGES) {
        if let Ok(Some(doc)) = store.load::<Database>(DocKey::Database(database)) {
            walk_database(&doc);
        }
    }
});
