//! A select or multi-select column's options: adding, renaming, recolouring, moving and deleting them (E09-S02,
//! E09-S11). They live in a movable list in the column's schema, each a map with an ID, a name and a colour name.

use leaf_core::{ColumnId, OptionId};
use loro::{Container, LoroMap, LoroMovableList, LoroValue, ValueOrContainer};

use super::property::keys;
use super::{Database, DatabaseOutcome};
use crate::ModelError;
use crate::page_meta::value;

impl Database {
    /// The column's options list (made with the column).
    pub(super) fn options_of(&self, column: ColumnId) -> Result<LoroMovableList, ModelError> {
        let current = self.column(column)?;
        if !current.kind.has_options() {
            return Err(ModelError::NotASelectColumn(column));
        }
        let schema = self.schema_of(column).ok_or(ModelError::ColumnNotFound(column))?;
        Ok(match schema.get(keys::OPTIONS) {
            Some(ValueOrContainer::Container(Container::MovableList(list))) => list,
            _ => schema.insert_container(keys::OPTIONS, LoroMovableList::new())?,
        })
    }

    pub(super) fn add_option(
        &mut self,
        column: ColumnId,
        name: &str,
        color: &str,
    ) -> Result<DatabaseOutcome, ModelError> {
        let list = self.options_of(column)?;
        let option = OptionId::new();
        let map = list.push_container(LoroMap::new())?;
        map.insert(keys::ID, option.to_string())?;
        map.insert(keys::NAME, name)?;
        map.insert(keys::COLOR, color)?;
        Ok(DatabaseOutcome::OptionCreated(option))
    }

    /// Positions in the options list holding `option` (more than one only after two lists merged).
    fn option_maps(&self, column: ColumnId, option: OptionId) -> Result<Vec<(usize, LoroMap)>, ModelError> {
        let list = self.options_of(column)?;
        let id = option.to_string();
        let found: Vec<(usize, LoroMap)> = (0..list.len())
            .filter_map(|i| match list.get(i) {
                Some(ValueOrContainer::Container(Container::Map(map)))
                    if value(&map, keys::ID) == Some(LoroValue::from(id.as_str())) =>
                {
                    Some((i, map))
                }
                _ => None,
            })
            .collect();
        if found.is_empty() {
            return Err(ModelError::OptionNotFound { column, option });
        }
        Ok(found)
    }

    pub(super) fn set_option(
        &mut self,
        column: ColumnId,
        option: OptionId,
        key: &str,
        text: &str,
    ) -> Result<DatabaseOutcome, ModelError> {
        for (_, map) in self.option_maps(column, option)? {
            if value(&map, key) != Some(LoroValue::from(text)) {
                map.insert(key, text)?;
            }
        }
        Ok(DatabaseOutcome::Done)
    }

    pub(super) fn delete_option(
        &mut self,
        column: ColumnId,
        option: OptionId,
    ) -> Result<DatabaseOutcome, ModelError> {
        let list = self.options_of(column)?;
        // Last first, so earlier positions stay right.
        for (at, _) in self.option_maps(column, option)?.into_iter().rev() {
            list.delete(at, 1)?;
        }
        Ok(DatabaseOutcome::Done)
    }

    /// Moves `option` to position `index` of the column's options as they read (each option once).
    pub(super) fn move_option(
        &mut self,
        column: ColumnId,
        option: OptionId,
        index: usize,
    ) -> Result<DatabaseOutcome, ModelError> {
        let list = self.options_of(column)?;
        let order = self.column(column)?.options;
        let Some(target) = order.get(index) else {
            return Err(ModelError::IndexOutOfRange {
                index,
                len: order.len(),
            });
        };
        // Positions in the stored list: an option listed twice (two copies merged) moves by its first entry, which is
        // the one that reads.
        let first = |id: OptionId| -> Result<usize, ModelError> {
            self.option_maps(column, id)?
                .first()
                .map(|(at, _)| *at)
                .ok_or(ModelError::OptionNotFound { column, option: id })
        };
        let from = first(option)?;
        let to = first(target.id)?;
        if from == to {
            return Ok(DatabaseOutcome::Unchanged);
        }
        list.mov(from, to)?;
        Ok(DatabaseOutcome::Done)
    }
}
