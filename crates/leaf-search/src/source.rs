//! Where the indexer reads saved documents from.
//!
//! A trait rather than `leaf-store` itself, so this crate depends only on the model (rule 4 of the
//! dependency rules) and tests can use documents held in memory.

use std::collections::HashMap;

use leaf_core::PageId;
use leaf_model::{PageDoc, Workspace};

use crate::PageVersion;

/// Any error from a [`PageSource`]; the indexer only logs it and tries again.
pub type SourceError = Box<dyn std::error::Error + Send + Sync>;

/// The saved version of the workspace document and of every saved page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavedVersions {
    /// The workspace document (titles, tree, trash), or `None` if never saved.
    pub workspace: Option<PageVersion>,
    /// Each page whose blocks were ever saved.
    pub pages: HashMap<PageId, PageVersion>,
}

/// Read access to the saved documents of one workspace.
///
/// The indexer only ever reads what is saved, never the editor's unsaved state, so the index can
/// always be compared with, and rebuilt from, what is on disk. `Send` because it runs on the
/// indexer's own thread.
pub trait PageSource: Send {
    /// Which version of each document is saved: cheap, read before anything else.
    fn versions(&mut self) -> Result<SavedVersions, SourceError>;
    /// The saved workspace document, or an empty one if it was never saved.
    fn workspace(&mut self) -> Result<Workspace, SourceError>;
    /// The saved blocks of `page`, or `None` if they were never saved (a new, empty page).
    fn page(&mut self, page: PageId) -> Result<Option<PageDoc>, SourceError>;
}
