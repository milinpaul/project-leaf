//! Writing a fixture to a workspace folder, so it can be opened in Leaf, indexed by search, or loaded
//! by a benchmark exactly as a person's workspace would be.

use std::path::Path;

use leaf_store::{DocKey, Workspace as Folder};

use crate::workspace::CREATED_AT;
use crate::{Fixture, FixtureError};

/// Creates a new workspace folder at `path` (it must not exist, or be empty) holding `fixture`, and
/// returns it open. Each document is saved once, as a single snapshot.
pub fn save_to_folder(path: &Path, fixture: &Fixture) -> Result<Folder, FixtureError> {
    let mut folder = Folder::create(path)?;
    let store = folder.store_mut();
    store.save(DocKey::Workspace, &fixture.workspace, CREATED_AT)?;
    for (page, doc) in &fixture.pages {
        store.save(DocKey::Page(*page), doc, CREATED_AT)?;
    }
    Ok(folder)
}
