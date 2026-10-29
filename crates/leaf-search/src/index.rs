//! The index file: opening (and recreating when unusable) and writing pages.

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use std::str::FromStr;

use leaf_core::PageId;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::schema::{COMPLETE_KEY, CREATE, INDEX_VERSION, VERSION_KEY};
use crate::search::{PageHit, search};
use crate::{PageText, PageVersion, SearchError};

/// Folder inside the workspace that holds the index. Matches `leaf_store::SEARCH`, which creates
/// it; repeated here because this crate must not depend on the store.
pub const SEARCH_DIR: &str = "search";

/// The index file inside [`SEARCH_DIR`].
pub const INDEX_FILE: &str = "index.sqlite";

/// Whether an opened index can be trusted to match the documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    /// The last full rebuild finished; since then pages are kept current one at a time.
    Current,
    /// The index is new, was recreated, or a rebuild was interrupted. The caller indexes every
    /// page, then calls [`SearchIndex::mark_complete`].
    NeedsRebuild,
}

/// The open search index of one workspace.
///
/// It only ever receives copies of text, never the documents, so nothing here can change or lose
/// a page. If the file is unusable it is thrown away and rebuilt.
#[derive(Debug)]
pub struct SearchIndex {
    conn: Connection,
    path: PathBuf,
}

/// What the index holds for one page: enough to tell whether it is out of date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedPage {
    /// The title as indexed.
    pub title: String,
    /// The save the indexed blocks came from.
    pub version: PageVersion,
}

impl SearchIndex {
    /// Opens the index of the workspace folder at `workspace_root`, creating `search/` and the
    /// file if they are missing.
    ///
    /// A file that can't be read, isn't a database, or has another [`INDEX_VERSION`] is deleted
    /// and replaced by an empty one. The returned [`IndexState`] says whether a rebuild is due.
    pub fn open(workspace_root: &Path) -> Result<(Self, IndexState), SearchError> {
        let dir = workspace_root.join(SEARCH_DIR);
        fs::create_dir_all(&dir)?;
        let path = dir.join(INDEX_FILE);
        if path.exists() {
            match Self::open_existing(&path) {
                Ok(Some((index, true))) => return Ok((index, IndexState::Current)),
                Ok(Some((index, false))) => return Ok((index, IndexState::NeedsRebuild)),
                Ok(None) => tracing::info!("search index has another version; rebuilding it"),
                Err(err) => tracing::warn!(%err, "search index is unreadable; rebuilding it"),
            }
            remove_index_files(&path)?;
        }
        Ok((Self::create(&path)?, IndexState::NeedsRebuild))
    }

    /// Replaces everything stored for `text.page` with `text`, taken from the save `version`, in
    /// one transaction.
    pub fn index_page(&mut self, text: &PageText, version: PageVersion) -> Result<(), SearchError> {
        let tx = self.conn.transaction()?;
        let page_id = text.page.to_string();
        delete_page(&tx, &page_id)?;
        tx.execute(
            "INSERT INTO pages (page_id, title, saved_at, saved_seq) VALUES (?1, ?2, ?3, ?4)",
            params![page_id, text.title, version.saved_at, version.seq],
        )?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO blocks (page_id, block_id, kind, text) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for block in &text.blocks {
                insert.execute(params![page_id, block.block.to_string(), block.kind, block.text])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Removes `page` from the index (trashed or deleted). Removing an unknown page does nothing.
    pub fn remove_page(&mut self, page: PageId) -> Result<(), SearchError> {
        let tx = self.conn.transaction()?;
        delete_page(&tx, &page.to_string())?;
        tx.commit()?;
        Ok(())
    }

    /// Empties the index and marks it as needing a rebuild, in one transaction.
    pub fn clear(&mut self) -> Result<(), SearchError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM blocks", [])?;
        tx.execute("DELETE FROM pages", [])?;
        set_meta(&tx, COMPLETE_KEY, "0")?;
        tx.commit()?;
        Ok(())
    }

    /// Records that a full rebuild finished, so the next [`SearchIndex::open`] reports
    /// [`IndexState::Current`]. Until this is called, a crash mid-rebuild means another rebuild.
    pub fn mark_complete(&mut self) -> Result<(), SearchError> {
        set_meta(&self.conn, COMPLETE_KEY, "1")?;
        Ok(())
    }

    /// Every indexed page with its title and version, to compare with the documents.
    ///
    /// A row whose page ID doesn't parse (which this crate never writes) is left out, so the next
    /// reconcile sees that page as missing and indexes it again.
    pub fn indexed_pages(&self) -> Result<HashMap<PageId, IndexedPage>, SearchError> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT page_id, title, saved_at, saved_seq FROM pages")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                IndexedPage {
                    title: row.get(1)?,
                    version: PageVersion {
                        saved_at: row.get(2)?,
                        seq: row.get(3)?,
                    },
                },
            ))
        })?;
        let mut pages = HashMap::new();
        for row in rows {
            let (id, page) = row?;
            if let Ok(id) = PageId::from_str(&id) {
                pages.insert(id, page);
            }
        }
        Ok(pages)
    }

    /// Runs a search on this connection. The search bar uses [`crate::Searcher`] instead, a
    /// separate read-only connection that can search while this one writes.
    pub fn search(&self, query: &str) -> Result<Vec<PageHit>, SearchError> {
        search(&self.conn, query)
    }

    /// Starts again empty after an error, since anything in the index can be rebuilt. Afterwards
    /// the index needs a rebuild.
    ///
    /// First drops and recreates every table inside the open file, which works even while the
    /// search bar has the file open (Windows won't delete an open file). Only if that fails, for
    /// example because the file isn't a database any more, is the file deleted and made anew.
    pub fn recreate(&mut self) -> Result<(), SearchError> {
        match reset_in_place(&mut self.conn) {
            Ok(()) => Ok(()),
            Err(err) => {
                tracing::info!(%err, "could not reset the search index in place; replacing the file");
                // Close the file before deleting it.
                drop(std::mem::replace(&mut self.conn, Connection::open_in_memory()?));
                remove_index_files(&self.path)?;
                *self = Self::create(&self.path)?;
                Ok(())
            }
        }
    }

    /// Opens an existing file: `None` if it has another version, otherwise the index and whether
    /// its last rebuild finished.
    fn open_existing(path: &Path) -> Result<Option<(Self, bool)>, SearchError> {
        let conn = connect(path)?;
        if get_meta(&conn, VERSION_KEY)? != Some(INDEX_VERSION.to_string()) {
            return Ok(None);
        }
        let complete = get_meta(&conn, COMPLETE_KEY)?.as_deref() == Some("1");
        Ok(Some((
            Self {
                conn,
                path: path.to_owned(),
            },
            complete,
        )))
    }

    /// Creates a fresh, empty index file at `path`.
    fn create(path: &Path) -> Result<Self, SearchError> {
        let mut conn = connect(path)?;
        let tx = conn.transaction()?;
        tx.execute_batch(CREATE)?;
        set_meta(&tx, VERSION_KEY, &INDEX_VERSION.to_string())?;
        set_meta(&tx, COMPLETE_KEY, "0")?;
        tx.commit()?;
        Ok(Self {
            conn,
            path: path.to_owned(),
        })
    }
}

/// Drops every table, index and trigger in the file and creates the schema again, empty and due a
/// rebuild, in one transaction. Full-text tables go first: dropping one also drops its own
/// internal tables, which is why the rest use `IF EXISTS`.
fn reset_in_place(conn: &mut Connection) -> Result<(), SearchError> {
    let tx = conn.transaction()?;
    let objects: Vec<(String, String)> = {
        let mut stmt = tx.prepare(
            "SELECT type, name FROM sqlite_master
             WHERE name NOT LIKE 'sqlite_%' AND type IN ('table', 'index', 'trigger', 'view')
             ORDER BY coalesce(sql, '') NOT LIKE 'CREATE VIRTUAL TABLE%'",
        )?;
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    for (kind, name) in objects {
        let kind = kind.to_uppercase();
        tx.execute_batch(&format!(
            "DROP {kind} IF EXISTS \"{}\"",
            name.replace('"', "\"\"")
        ))?;
    }
    tx.execute_batch(CREATE)?;
    set_meta(&tx, VERSION_KEY, &INDEX_VERSION.to_string())?;
    set_meta(&tx, COMPLETE_KEY, "0")?;
    tx.commit()?;
    Ok(())
}

/// Opens a connection. WAL lets searches read while a page is being re-indexed; `NORMAL` sync is
/// enough because a lost or damaged index is simply rebuilt.
fn connect(path: &Path) -> Result<Connection, SearchError> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

/// Deletes the index file and SQLite's side files, so a fresh index can't pick up stale ones.
fn remove_index_files(path: &Path) -> Result<(), SearchError> {
    for suffix in ["", "-wal", "-shm"] {
        let mut file = PathBuf::from(path);
        file.as_mut_os_string().push(suffix);
        match fs::remove_file(&file) {
            Err(err) if err.kind() != ErrorKind::NotFound => return Err(err.into()),
            _ => {}
        }
    }
    Ok(())
}

fn delete_page(tx: &Transaction<'_>, page_id: &str) -> Result<(), SearchError> {
    tx.execute("DELETE FROM blocks WHERE page_id = ?1", [page_id])?;
    tx.execute("DELETE FROM pages WHERE page_id = ?1", [page_id])?;
    Ok(())
}

fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, SearchError> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get(0))
        .optional()?)
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), SearchError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}
