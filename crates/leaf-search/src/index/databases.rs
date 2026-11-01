//! A database's cells in the index (E09-S10): which save of each database was read, and its text cells as
//! `property` entries under their rows' pages. Updating a database writes only the cells whose text changed, so a
//! one-cell edit in a 10,000-row database touches one entry.

use std::collections::HashMap;
use std::str::FromStr;

use leaf_core::DatabaseId;
use rusqlite::params;

use super::SearchIndex;
use crate::{CellText, PROPERTY_KIND, PageVersion, SearchError};

/// The `block_id` of a cell's entry: never a block's, so a hit on it scrolls nowhere.
fn cell_key(database: DatabaseId, cell: &CellText) -> String {
    format!("{database}/{}", cell.column)
}

impl SearchIndex {
    /// Every database whose cells are indexed, with the save they were read from.
    pub fn indexed_databases(&self) -> Result<HashMap<DatabaseId, PageVersion>, SearchError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT database_id, saved_at, saved_seq FROM databases")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                PageVersion {
                    saved_at: row.get(1)?,
                    seq: row.get(2)?,
                },
            ))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (id, version) = row?;
            if let Ok(id) = DatabaseId::from_str(&id) {
                out.insert(id, version);
            }
        }
        Ok(out)
    }

    /// Makes the indexed cells of `database` exactly `cells`, read from save `version`, in one transaction: cells
    /// whose text changed are rewritten, gone ones removed, new ones added; the rest are left alone.
    pub fn index_database(
        &mut self,
        database: DatabaseId,
        version: PageVersion,
        cells: &[CellText],
    ) -> Result<(), SearchError> {
        let tx = self.conn.transaction()?;
        let prefix = format!("{database}/");
        let mut have: HashMap<(String, String), (i64, String)> = HashMap::new();
        {
            let mut stmt = tx.prepare_cached(
                "SELECT id, page_id, block_id, text FROM blocks WHERE kind = ?1 AND substr(block_id, 1, ?2) = ?3",
            )?;
            let width = i64::try_from(prefix.chars().count()).unwrap_or(i64::MAX);
            let rows = stmt.query_map(params![PROPERTY_KIND, width, prefix], |row| {
                Ok((
                    (row.get::<_, String>(1)?, row.get::<_, String>(2)?),
                    (row.get::<_, i64>(0)?, row.get::<_, String>(3)?),
                ))
            })?;
            for row in rows {
                let (key, value) = row?;
                have.insert(key, value);
            }
            let mut insert = tx.prepare_cached(
                "INSERT INTO blocks (page_id, block_id, kind, text) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut update = tx.prepare_cached("UPDATE blocks SET text = ?2 WHERE id = ?1")?;
            for cell in cells {
                let key = (cell.page.to_string(), cell_key(database, cell));
                match have.remove(&key) {
                    Some((_, text)) if text == cell.text => {}
                    Some((id, _)) => {
                        update.execute(params![id, cell.text])?;
                    }
                    None => {
                        insert.execute(params![key.0, key.1, PROPERTY_KIND, cell.text])?;
                    }
                }
            }
            let mut delete = tx.prepare_cached("DELETE FROM blocks WHERE id = ?1")?;
            for (id, _) in have.into_values() {
                delete.execute([id])?;
            }
        }
        tx.execute(
            "INSERT INTO databases (database_id, saved_at, saved_seq) VALUES (?1, ?2, ?3)
             ON CONFLICT (database_id) DO UPDATE SET saved_at = excluded.saved_at, saved_seq = excluded.saved_seq",
            params![database.to_string(), version.saved_at, version.seq],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Forgets `database` and its cells (its page went to the Trash, or was deleted).
    pub fn remove_database(&mut self, database: DatabaseId) -> Result<(), SearchError> {
        self.index_database(database, PageVersion::default(), &[])?;
        self.conn.execute(
            "DELETE FROM databases WHERE database_id = ?1",
            [database.to_string()],
        )?;
        Ok(())
    }

    /// The pages whose cells are indexed under `database`, for tests.
    #[cfg(test)]
    pub(crate) fn cell_pages(&self, database: DatabaseId) -> Result<Vec<leaf_core::PageId>, SearchError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT page_id FROM blocks WHERE kind = ?1 AND block_id LIKE ?2")?;
        let rows = stmt.query_map(params![PROPERTY_KIND, format!("{database}/%")], |row| {
            row.get::<_, String>(0)
        })?;
        let mut out = Vec::new();
        for row in rows {
            if let Ok(page) = leaf_core::PageId::from_str(&row?) {
                out.push(page);
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use leaf_core::{ColumnId, DatabaseId, PageId};

    use crate::{CellText, PageVersion, SearchIndex};

    fn index() -> (SearchIndex, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("leaf-search-cells-{}", PageId::new()));
        let (index, _) = SearchIndex::open(&dir).unwrap();
        (index, dir)
    }

    fn row_ids(index: &SearchIndex) -> Vec<(i64, String)> {
        let mut stmt = index
            .conn
            .prepare("SELECT id, text FROM blocks WHERE kind = 'property' ORDER BY id")
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn only_changed_cells_are_written() {
        let (mut index, dir) = index();
        let (db, other) = (DatabaseId::new(), DatabaseId::new());
        let (a, b) = (PageId::new(), PageId::new());
        let column = ColumnId::new();
        let cell = |page, text: &str| CellText {
            page,
            column,
            text: text.into(),
        };
        let v = |seq| PageVersion { saved_at: seq, seq };
        index
            .index_database(db, v(1), &[cell(a, "one"), cell(b, "two")])
            .unwrap();
        index
            .index_database(other, v(1), &[cell(a, "elsewhere")])
            .unwrap();
        let before = row_ids(&index);

        index
            .index_database(db, v(2), &[cell(a, "one"), cell(b, "three")])
            .unwrap();
        let after = row_ids(&index);
        assert_eq!(after[0], before[0], "the unchanged cell keeps its entry");
        assert_eq!(after[1].1, "three", "the changed one is rewritten in place");
        assert_eq!(after[2], before[2], "another database's cells are left alone");
        assert_eq!(index.indexed_databases().unwrap()[&db], v(2));

        index.index_database(db, v(3), &[cell(b, "three")]).unwrap();
        assert_eq!(index.cell_pages(db).unwrap(), vec![b], "a gone cell is removed");
        index.remove_database(db).unwrap();
        assert_eq!(index.cell_pages(db).unwrap(), Vec::<PageId>::new());
        assert!(!index.indexed_databases().unwrap().contains_key(&db));
        assert_eq!(index.cell_pages(other).unwrap(), vec![a]);
        drop(index);
        let _ = std::fs::remove_dir_all(dir);
    }
}
