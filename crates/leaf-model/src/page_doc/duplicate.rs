//! Duplicating blocks: a deep copy of each block and everything nested in it, right after the original.
//!
//! The copy is made field by field from the stored data, not from what this version understands: every
//! entry of a block's record is copied, including properties and formatting from newer versions, so
//! duplicating never loses anything (architecture rule 5). Only the block IDs are new.

use leaf_core::BlockId;
use loro::{Container, LoroList, LoroMap, LoroText, TreeID, ValueOrContainer};

use super::PageDoc;
use super::split_merge::append_delta;
use crate::block_kind::keys;
use crate::{ModelError, PageOutcome};

impl PageDoc {
    pub(super) fn duplicate_blocks(&mut self, ids: &[BlockId]) -> Result<PageOutcome, ModelError> {
        let roots = self.outermost(ids)?;
        self.refuse_table_parts(&roots)?;
        if roots.is_empty() {
            return Ok(PageOutcome::Unchanged);
        }
        let mut created = Vec::with_capacity(roots.len());
        for root in roots {
            let parent = self.parent_node(root);
            let siblings = self.tree.children(parent).unwrap_or_default();
            let index = siblings
                .iter()
                .position(|&n| n == root)
                .map_or(siblings.len(), |p| p + 1);
            created.push(self.copy_subtree(root, parent, index)?);
        }
        Ok(PageOutcome::BlocksCreated(created))
    }

    fn copy_subtree(
        &mut self,
        source: TreeID,
        parent: Option<TreeID>,
        index: usize,
    ) -> Result<BlockId, ModelError> {
        let block = BlockId::new();
        let node = self.tree.create_at(parent, index)?;
        let from = self.tree.get_meta(source)?;
        let to = self.tree.get_meta(node)?;
        for (key, value) in entries(&from) {
            match (key.as_str(), value) {
                (keys::ID, _) => {}
                // The block text keeps the container kind every block's text has (ADR-001).
                (keys::TEXT, ValueOrContainer::Container(Container::Text(text))) => {
                    append_delta(&to.ensure_mergeable_text(keys::TEXT)?, text.to_delta())?;
                }
                (key, value) => copy_into_map(&to, key, value)?,
            }
        }
        to.insert(keys::ID, block.to_string())?;
        self.index.insert(block, node);
        for (i, child) in self
            .tree
            .children(source)
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            self.copy_subtree(child, Some(node), i)?;
        }
        Ok(block)
    }
}

fn entries(map: &LoroMap) -> Vec<(String, ValueOrContainer)> {
    let mut out = Vec::new();
    map.for_each(|key, value| out.push((key.to_owned(), value)));
    out
}

/// Copies one value, containers included, under `key` in `map`.
fn copy_into_map(map: &LoroMap, key: &str, value: ValueOrContainer) -> Result<(), ModelError> {
    match value {
        ValueOrContainer::Value(value) => {
            map.insert(key, value)?;
        }
        ValueOrContainer::Container(container) => match container {
            Container::Text(text) => copy_text(&text, &map.insert_container(key, LoroText::new())?)?,
            Container::Map(inner) => copy_map(&inner, &map.insert_container(key, LoroMap::new())?)?,
            Container::List(inner) => copy_list(&inner, &map.insert_container(key, LoroList::new())?)?,
            // Kinds a block record has never held: keep their contents as a plain value.
            other => {
                map.insert(key, ValueOrContainer::Container(other).get_deep_value())?;
            }
        },
    }
    Ok(())
}

fn copy_map(from: &LoroMap, to: &LoroMap) -> Result<(), ModelError> {
    for (key, value) in entries(from) {
        copy_into_map(to, &key, value)?;
    }
    Ok(())
}

fn copy_list(from: &LoroList, to: &LoroList) -> Result<(), ModelError> {
    let mut items = Vec::new();
    from.for_each(|value| items.push(value));
    for (i, value) in items.into_iter().enumerate() {
        match value {
            ValueOrContainer::Value(value) => to.insert(i, value)?,
            ValueOrContainer::Container(Container::Text(text)) => {
                copy_text(&text, &to.insert_container(i, LoroText::new())?)?;
            }
            ValueOrContainer::Container(Container::Map(inner)) => {
                copy_map(&inner, &to.insert_container(i, LoroMap::new())?)?;
            }
            ValueOrContainer::Container(Container::List(inner)) => {
                copy_list(&inner, &to.insert_container(i, LoroList::new())?)?;
            }
            other @ ValueOrContainer::Container(_) => to.insert(i, other.get_deep_value())?,
        }
    }
    Ok(())
}

fn copy_text(from: &LoroText, to: &LoroText) -> Result<(), ModelError> {
    append_delta(to, from.to_delta())
}

#[cfg(test)]
mod tests;
