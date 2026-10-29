//! The search indexer's view of the saved workspace: `leaf-store` on one side, the
//! `leaf_search::PageSource` trait on the other. Pure translation, no decisions.

use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace};
use leaf_search::{PageSource, PageVersion, SavedVersions, SourceError};
use leaf_store::{DocKey, SavedVersion, Store};

/// A second connection to the workspace database, used only by the indexer's thread.
pub struct StoreSource(pub Store);

fn version(saved: SavedVersion) -> PageVersion {
    PageVersion {
        saved_at: saved.saved_at,
        seq: saved.seq,
    }
}

impl PageSource for StoreSource {
    fn versions(&mut self) -> Result<SavedVersions, SourceError> {
        let mut versions = SavedVersions::default();
        for (key, saved) in self.0.saved_versions()? {
            match key {
                DocKey::Workspace => versions.workspace = Some(version(saved)),
                DocKey::Page(page) => {
                    versions.pages.insert(page, version(saved));
                }
            }
        }
        Ok(versions)
    }

    fn workspace(&mut self) -> Result<Workspace, SourceError> {
        Ok(self.0.load(DocKey::Workspace)?.unwrap_or_default())
    }

    fn page(&mut self, page: PageId) -> Result<Option<PageDoc>, SourceError> {
        Ok(self.0.load(DocKey::Page(page))?)
    }
}
