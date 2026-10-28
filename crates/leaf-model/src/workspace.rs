//! The workspace document: the tree of pages, their titles, icons and trash state.
//!
//! One `Workspace` owns one Loro document holding the `"pages"` tree. Everything outside this module
//! talks about pages by `PageId`; Loro's `TreeID`s stay private because undo and redo can re-create
//! nodes under new ones (ADR 0006).

use std::collections::HashMap;

use leaf_core::PageId;
use loro::{ExportMode, LoroDoc, LoroTree, LoroValue, TreeID, UndoManager};

use crate::page_meta::{keys, value};
use crate::saved::{self, SavedVersion};
use crate::{ModelError, PageMeta};

/// Name of the Loro tree that holds the page hierarchy.
const PAGES: &str = "pages";

/// A workspace's page tree and the metadata of every page in it.
///
/// Changed only through [`Workspace::apply`] and its [`WorkspaceCommand`](crate::WorkspaceCommand)s.
pub struct Workspace {
    doc: LoroDoc,
    tree: LoroTree,
    index: PageIndex,
    undo: UndoManager,
    saved: SavedVersion,
}

/// Two-way map between permanent `PageId`s and Loro's current `TreeID`s, for O(1) lookups.
#[derive(Default)]
struct PageIndex {
    by_page: HashMap<PageId, TreeID>,
    by_node: HashMap<TreeID, PageId>,
}

impl Workspace {
    /// An empty workspace with no pages.
    #[must_use]
    pub fn new() -> Self {
        let doc = LoroDoc::new();
        let tree = pages_tree(&doc);
        let undo = undo::manager(&doc);
        let saved = SavedVersion::of(&doc);
        Self {
            doc,
            tree,
            index: PageIndex::default(),
            undo,
            saved,
        }
    }

    /// Loads a workspace from bytes produced by [`Workspace::to_bytes`] (or any Loro export of one).
    ///
    /// Fails if any page lacks a valid, unique `id`, rather than hiding that page.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        Self::from_saved(bytes, &[])
    }

    /// Loads a workspace the way the store keeps it: a snapshot plus the updates saved after it, in
    /// order. The loaded state counts as saved.
    pub fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError> {
        let doc = LoroDoc::new();
        saved::import(&doc, snapshot, updates)?;
        Self::from_doc(doc)
    }

    /// Loads what can be loaded after [`Workspace::from_saved`] failed: the snapshot plus every
    /// update that isn't damaged. Returns the workspace and the positions in `updates` that were
    /// skipped, so the caller can warn. A damaged snapshot is still an error.
    pub fn recover(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(Self, Vec<usize>), ModelError> {
        let doc = LoroDoc::new();
        let skipped = saved::import_each(&doc, snapshot, updates)?;
        Ok((Self::from_doc(doc)?, skipped))
    }

    fn from_doc(doc: LoroDoc) -> Result<Self, ModelError> {
        let tree = pages_tree(&doc);
        let index = PageIndex::build(&tree)?;
        // Created after the import, so undo starts from the loaded state.
        let undo = undo::manager(&doc);
        let saved = SavedVersion::of(&doc);
        Ok(Self {
            doc,
            tree,
            index,
            undo,
            saved,
        })
    }

    /// The whole workspace as a self-contained snapshot, ready to save.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Every change since the last [`Workspace::mark_saved`] (or load) as one update to append to
    /// the saved workspace, or `None` when nothing changed. Undo and redo count as changes.
    pub fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError> {
        self.saved.changes(&self.doc)
    }

    /// Records that the workspace as it is now is on disk. Call only after the write succeeded, so a
    /// failed save is retried with the same changes next time. Needs only shared access: it changes
    /// no content.
    pub fn mark_saved(&self) {
        self.saved.update(&self.doc);
    }

    /// Whether anything changed since the last save (or load). Cheap enough to poll from a timer.
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        self.saved.is_behind(&self.doc)
    }

    /// Top-level pages, in sidebar order. Pages in the trash are left out, and with them everything
    /// inside them; [`Workspace::trash`] lists those.
    pub fn root_pages(&self) -> Result<Vec<PageId>, ModelError> {
        self.visible(self.tree.roots())
    }

    /// Child pages of `page`, in order, leaving out pages in the trash.
    pub fn children(&self, page: PageId) -> Result<Vec<PageId>, ModelError> {
        let node = self.node(page)?;
        self.visible(self.tree.children(node).unwrap_or_default())
    }

    /// Title, icon and dates of `page`.
    pub fn page(&self, page: PageId) -> Result<PageMeta, ModelError> {
        let meta = self.tree.get_meta(self.node(page)?)?;
        PageMeta::read(page, &meta)
    }

    /// Whether `page` exists in this workspace (pages in the trash included).
    #[must_use]
    pub fn contains(&self, page: PageId) -> bool {
        self.index.by_page.contains_key(&page)
    }

    /// Number of pages, trashed ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.by_page.len()
    }

    /// Whether the workspace has no pages at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.by_page.is_empty()
    }

    /// Siblings as pages, without those that were trashed themselves.
    fn visible(&self, nodes: Vec<TreeID>) -> Result<Vec<PageId>, ModelError> {
        let mut pages = Vec::with_capacity(nodes.len());
        for node in nodes {
            if !self.is_trashed(node)? {
                pages.extend(self.index.by_node.get(&node).copied());
            }
        }
        Ok(pages)
    }

    fn node(&self, page: PageId) -> Result<TreeID, ModelError> {
        self.index
            .by_page
            .get(&page)
            .copied()
            .ok_or(ModelError::PageNotFound(page))
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

/// The pages tree, with fractional-index ordering switched on. The setting is not stored in the
/// document, so it must be applied every time a doc is created or loaded.
fn pages_tree(doc: &LoroDoc) -> LoroTree {
    let tree = doc.get_tree(PAGES);
    tree.enable_fractional_index(0);
    tree
}

impl PageIndex {
    /// Reads every live page node's `id`. Called on load, and after undo and redo, which can
    /// re-create nodes under new `TreeID`s (ADR 0006).
    fn build(tree: &LoroTree) -> Result<Self, ModelError> {
        let mut index = Self::default();
        for node in tree.get_nodes(false) {
            let page = page_id_of(tree, node.id)?;
            if index.by_page.insert(page, node.id).is_some() {
                return Err(ModelError::DuplicatePage(page));
            }
            index.by_node.insert(node.id, page);
        }
        Ok(index)
    }

    /// Records a page just created by a command.
    fn insert(&mut self, page: PageId, node: TreeID) {
        self.by_page.insert(page, node);
        self.by_node.insert(node, page);
    }

    /// Forgets a page removed by a command.
    fn remove(&mut self, page: PageId) {
        if let Some(node) = self.by_page.remove(&page) {
            self.by_node.remove(&node);
        }
    }
}

fn page_id_of(tree: &LoroTree, node: TreeID) -> Result<PageId, ModelError> {
    let invalid = |reason: String| ModelError::InvalidPage {
        node: format!("{node:?}"),
        reason,
    };
    match value(&tree.get_meta(node)?, keys::ID) {
        Some(LoroValue::String(text)) => text
            .parse()
            .map_err(|e: leaf_core::CoreError| invalid(e.to_string())),
        Some(_) => Err(invalid("id is not text".to_owned())),
        None => Err(invalid("id is missing".to_owned())),
    }
}

mod apply;
mod trash;
mod undo;

pub use undo::DEFAULT_UNDO_MERGE_MS;

#[cfg(test)]
mod tests;
