//! A page's own document: the tree of blocks with their kinds and text.
//!
//! One `PageDoc` owns one Loro document holding the `"blocks"` tree. Like the workspace, it talks
//! about blocks by permanent `BlockId`s and keeps Loro's `TreeID`s private, because undo and redo
//! can re-create nodes under new ones (ADR 0006).

use std::collections::HashMap;
use std::sync::Arc;

use leaf_core::BlockId;
use loro::{
    Container, ExportMode, LoroDoc, LoroText, LoroTree, LoroValue, TreeID, TreeParentId, UndoManager,
    ValueOrContainer,
};

use crate::block_kind::keys;
use crate::page_meta::value;
use crate::saved::{self, SavedVersion};
use crate::text::{configure_marks, spans};
use crate::{BlockKind, ModelError, TextSpan};

/// Name of the Loro tree that holds a page's blocks.
const BLOCKS: &str = "blocks";

/// The blocks of one page. Changed only through [`PageDoc::apply`] and its
/// [`PageCommand`](crate::PageCommand)s.
pub struct PageDoc {
    doc: LoroDoc,
    tree: LoroTree,
    index: BlockIndex,
    undo: UndoManager,
    saved: SavedVersion,
    pending: Arc<std::sync::Mutex<changes::Pending>>,
    _events: loro::Subscription,
    carets: Arc<undo::CaretSlots>,
}

/// Two-way map between permanent `BlockId`s and Loro's current `TreeID`s, for O(1) lookups.
#[derive(Default)]
struct BlockIndex {
    by_block: HashMap<BlockId, TreeID>,
    by_node: HashMap<TreeID, BlockId>,
}

impl PageDoc {
    /// An empty page with no blocks.
    #[must_use]
    pub fn new() -> Self {
        let doc = new_doc();
        let tree = blocks_tree(&doc);
        let carets = Arc::new(undo::CaretSlots::default());
        let undo = undo::manager(&doc, &carets);
        let saved = SavedVersion::of(&doc);
        let pending = Arc::default();
        let events = changes::watch(&doc, &pending);
        Self {
            doc,
            tree,
            index: BlockIndex::default(),
            undo,
            saved,
            pending,
            _events: events,
            carets,
        }
    }

    /// Loads a page from bytes produced by [`PageDoc::to_bytes`] (or any Loro export of one).
    ///
    /// Fails if any block lacks a valid, unique `id`, rather than hiding it. Block kinds are read when
    /// asked for, so one malformed block does not stop the rest of the page from opening.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        Self::from_saved(bytes, &[])
    }

    /// Loads a page the way the store keeps it: a snapshot plus the updates saved after it, in
    /// order. The loaded state counts as saved.
    pub fn from_saved(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<Self, ModelError> {
        let doc = new_doc();
        saved::import(&doc, snapshot, updates)?;
        Self::from_doc(doc)
    }

    /// Loads what can be loaded after [`PageDoc::from_saved`] failed: the snapshot plus every
    /// update that isn't damaged. Returns the page and the positions in `updates` that were skipped,
    /// so the caller can warn. A damaged snapshot is still an error.
    pub fn recover(snapshot: &[u8], updates: &[Vec<u8>]) -> Result<(Self, Vec<usize>), ModelError> {
        let doc = new_doc();
        let skipped = saved::import_each(&doc, snapshot, updates)?;
        Ok((Self::from_doc(doc)?, skipped))
    }

    fn from_doc(doc: LoroDoc) -> Result<Self, ModelError> {
        let tree = blocks_tree(&doc);
        let index = BlockIndex::build(&tree)?;
        // Created after the import, so undo starts from the loaded state.
        let carets = Arc::new(undo::CaretSlots::default());
        let undo = undo::manager(&doc, &carets);
        let saved = SavedVersion::of(&doc);
        // Watching starts after the load, so loading itself isn't reported as a change.
        let pending = Arc::default();
        let events = changes::watch(&doc, &pending);
        Ok(Self {
            doc,
            tree,
            index,
            undo,
            saved,
            pending,
            _events: events,
            carets,
        })
    }

    /// The whole page as a self-contained snapshot, ready to save.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ModelError> {
        Ok(self.doc.export(ExportMode::Snapshot)?)
    }

    /// Every change since the last [`PageDoc::mark_saved`] (or load) as one update to append to
    /// the saved page, or `None` when nothing changed. Undo and redo count as changes.
    pub fn unsaved_changes(&self) -> Result<Option<Vec<u8>>, ModelError> {
        self.saved.changes(&self.doc)
    }

    /// Records that the page as it is now is on disk. Call only after the write succeeded, so a
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

    /// Top-level blocks, in order.
    #[must_use]
    pub fn root_blocks(&self) -> Vec<BlockId> {
        self.index.blocks_for(self.tree.roots())
    }

    /// Blocks nested directly inside `block`, in order.
    pub fn children(&self, block: BlockId) -> Result<Vec<BlockId>, ModelError> {
        Ok(self
            .index
            .blocks_for(self.tree.children(self.node(block)?).unwrap_or_default()))
    }

    /// The block `block` is nested in, or `None` at the top level.
    pub fn parent(&self, block: BlockId) -> Result<Option<BlockId>, ModelError> {
        Ok(match self.tree.parent(self.node(block)?) {
            Some(TreeParentId::Node(parent)) => self.index.by_node.get(&parent).copied(),
            _ => None,
        })
    }

    /// The block's kind and properties.
    pub fn kind(&self, block: BlockId) -> Result<BlockKind, ModelError> {
        let meta = self.tree.get_meta(self.node(block)?)?;
        BlockKind::read(&meta).map_err(|(field, reason)| ModelError::InvalidBlockData {
            block,
            field,
            reason,
        })
    }

    /// The block's text as runs of equally formatted characters, or `None` for blocks without text
    /// (dividers, unknown kinds).
    pub fn text(&self, block: BlockId) -> Result<Option<Vec<TextSpan>>, ModelError> {
        Ok(self.text_container(block)?.map(|text| spans(&text)))
    }

    /// The block's text without formatting, or `None` for blocks without text.
    pub fn plain_text(&self, block: BlockId) -> Result<Option<String>, ModelError> {
        Ok(self.text_container(block)?.map(|text| text.to_string()))
    }

    /// Whether `block` is on this page.
    #[must_use]
    pub fn contains(&self, block: BlockId) -> bool {
        self.index.by_block.contains_key(&block)
    }

    /// Number of blocks on the page, nested ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.by_block.len()
    }

    /// Whether the page has no blocks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.by_block.is_empty()
    }

    pub(super) fn text_container(&self, block: BlockId) -> Result<Option<LoroText>, ModelError> {
        Ok(match self.tree.get_meta(self.node(block)?)?.get(keys::TEXT) {
            Some(ValueOrContainer::Container(Container::Text(text))) => Some(text),
            _ => None,
        })
    }

    pub(super) fn node(&self, block: BlockId) -> Result<TreeID, ModelError> {
        self.index
            .by_block
            .get(&block)
            .copied()
            .ok_or(ModelError::BlockNotFound(block))
    }
}

impl Default for PageDoc {
    fn default() -> Self {
        Self::new()
    }
}

/// A document with the mark table applied. The setting is not stored, so every new or loaded doc
/// needs it before any edit.
fn new_doc() -> LoroDoc {
    let doc = LoroDoc::new();
    configure_marks(&doc);
    doc
}

/// The blocks tree with fractional-index ordering on (also not stored in the document).
fn blocks_tree(doc: &LoroDoc) -> LoroTree {
    let tree = doc.get_tree(BLOCKS);
    tree.enable_fractional_index(0);
    tree
}

impl BlockIndex {
    /// Reads every live block node's `id`. Called on load and after undo and redo.
    fn build(tree: &LoroTree) -> Result<Self, ModelError> {
        let mut index = Self::default();
        for node in tree.get_nodes(false) {
            let block = block_id_of(tree, node.id)?;
            if index.by_block.insert(block, node.id).is_some() {
                return Err(ModelError::DuplicateBlock(block));
            }
            index.by_node.insert(node.id, block);
        }
        Ok(index)
    }

    fn insert(&mut self, block: BlockId, node: TreeID) {
        self.by_block.insert(block, node);
        self.by_node.insert(node, block);
    }

    fn remove(&mut self, block: BlockId) {
        if let Some(node) = self.by_block.remove(&block) {
            self.by_node.remove(&node);
        }
    }

    /// Translates Loro nodes to blocks, keeping order. Every live node is indexed, so a miss would
    /// mean the doc changed outside the model; such nodes are skipped, not invented.
    fn blocks_for(&self, nodes: Vec<TreeID>) -> Vec<BlockId> {
        nodes
            .into_iter()
            .filter_map(|node| self.by_node.get(&node).copied())
            .collect()
    }
}

fn block_id_of(tree: &LoroTree, node: TreeID) -> Result<BlockId, ModelError> {
    let invalid = |reason: String| ModelError::InvalidBlock {
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
mod changes;
mod invariants;

pub use changes::Changes;
pub use invariants::InvariantViolation;
pub use undo::{Caret, History};

mod kind_ops;
mod nesting;
mod split_merge;
mod text_ops;
mod undo;

#[cfg(test)]
mod tests;
