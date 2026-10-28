//! The workspace document: the tree of pages, their titles, icons and trash state.
//!
//! One `Workspace` owns one Loro document holding the `"pages"` tree. Everything outside this module
//! talks about pages by `PageId`; Loro's `TreeID`s stay private because undo and redo can re-create
//! nodes under new ones (ADR 0006).

use std::collections::HashMap;

use leaf_core::PageId;
use loro::{ExportMode, LoroDoc, LoroTree, LoroValue, TreeID};

use crate::page_meta::{keys, value};
use crate::{ModelError, PageMeta};

/// Name of the Loro tree that holds the page hierarchy.
const PAGES: &str = "pages";

/// A workspace's page tree and the metadata of every page in it.
///
/// Read-only for now: the page commands (E01-S03 onwards) are the only code allowed to change it.
pub struct Workspace {
    doc: LoroDoc,
    tree: LoroTree,
    index: PageIndex,
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
        Self {
            doc,
            tree,
            index: PageIndex::default(),
        }
    }

    /// Loads a workspace from bytes produced by [`Workspace::to_bytes`] (or any Loro export of one).
    ///
    /// Fails if any page lacks a valid, unique `id`, rather than hiding that page.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        let doc = LoroDoc::new();
        doc.import(bytes)?;
        let tree = pages_tree(&doc);
        let index = PageIndex::build(&tree)?;
        Ok(Self { doc, tree, index })
    }

    /// The whole workspace as a self-contained snapshot, ready to save.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Top-level pages, in sidebar order. Includes trashed pages (check [`PageMeta::trashed_at`]).
    #[must_use]
    pub fn root_pages(&self) -> Vec<PageId> {
        self.index.pages_for(self.tree.roots())
    }

    /// Child pages of `page`, in order. Includes trashed pages.
    pub fn children(&self, page: PageId) -> Result<Vec<PageId>, ModelError> {
        let node = self.node(page)?;
        Ok(self.index.pages_for(self.tree.children(node).unwrap_or_default()))
    }

    /// Title, icon and dates of `page`.
    pub fn page(&self, page: PageId) -> Result<PageMeta, ModelError> {
        let meta = self.tree.get_meta(self.node(page)?)?;
        PageMeta::read(page, &meta)
    }

    /// Whether `page` exists in this workspace (trashed pages included).
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
    /// Reads every live page node's `id`. Called on load; undo will call it too (E01-S06).
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

    /// Translates Loro nodes to pages, keeping order. Every live node is indexed on load, so a
    /// miss would mean the doc changed outside the model; such nodes are skipped, not invented.
    fn pages_for(&self, nodes: Vec<TreeID>) -> Vec<PageId> {
        nodes
            .into_iter()
            .filter_map(|node| self.by_node.get(&node).copied())
            .collect()
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

#[cfg(test)]
mod tests;
