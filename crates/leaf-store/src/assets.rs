//! Image files kept with a workspace, named by the BLAKE3 hash of their bytes (`AssetId`).
//!
//! Importing hashes the bytes, writes them to `assets/<hash>` unless a correct copy is already
//! there, then records the file's type and size in the `assets` table. Naming by content means the
//! same picture pasted twice is stored once, and any file can be checked against its name. The file
//! is written before its row, and atomically (a temporary file renamed into place), so the table
//! never names a file that is missing or half written.
//!
//! Only images are accepted: their type is read from the first bytes of the file, not from its
//! name, so a renamed file can't pass for an image.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use leaf_core::AssetId;
use rusqlite::{OptionalExtension, params};

use crate::{Store, StoreError};

mod kind;

pub use kind::image_mime;

/// What the workspace knows about one stored image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    /// The hash of its bytes, which is also its file name in `assets/`.
    pub id: AssetId,
    /// Its media type, such as `image/png`, read from its content.
    pub mime: String,
    /// Its size in bytes.
    pub bytes: u64,
    /// When it was first imported, in Unix milliseconds. A later import of the same bytes keeps it.
    pub created_at: i64,
}

/// The `assets/` folder of one workspace and the table that describes it.
pub(crate) struct Assets<'a> {
    pub(crate) folder: &'a Path,
    pub(crate) store: &'a mut Store,
}

impl Assets<'_> {
    /// Stores `bytes` (an image) at `now` and returns what was recorded. Importing the same bytes
    /// again writes nothing new and returns the first record.
    pub(crate) fn import(&mut self, bytes: &[u8], now: i64) -> Result<Asset, StoreError> {
        let mime = image_mime(bytes).ok_or(StoreError::NotAnImage)?;
        let id = AssetId::of(bytes);
        let path = path_of(self.folder, id);
        if !holds(&path, id) {
            write_atomically(&path, bytes)?;
        }
        let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        self.store.conn.execute(
            "INSERT OR IGNORE INTO assets (asset_id, mime, bytes, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id.to_string(), mime, i64::try_from(size).unwrap_or(i64::MAX), now],
        )?;
        // The row exists now: this import's, or the first one's for the same bytes (kept as is).
        Ok(find(self.store, id)?.unwrap_or(Asset {
            id,
            mime: mime.to_owned(),
            bytes: size,
            created_at: now,
        }))
    }
}

/// The record for `id`, if it was imported.
pub(crate) fn find(store: &Store, id: AssetId) -> Result<Option<Asset>, StoreError> {
    let row = store
        .conn
        .query_row(
            "SELECT mime, bytes, created_at FROM assets WHERE asset_id = ?1",
            params![id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(row.map(|(mime, bytes, created_at)| Asset {
        id,
        mime,
        bytes: u64::try_from(bytes).unwrap_or(0),
        created_at,
    }))
}

/// Where the file for `id` lives inside `folder`.
pub(crate) fn path_of(folder: &Path, id: AssetId) -> PathBuf {
    folder.join(id.to_string())
}

/// Whether `path` already holds exactly the bytes named `id`. A damaged or truncated copy (from a
/// disk problem, or someone editing the folder) doesn't count, so the next import repairs it.
fn holds(path: &Path, id: AssetId) -> bool {
    fs::read(path).is_ok_and(|existing| AssetId::of(&existing) == id)
}

/// Writes `bytes` to `path` through a temporary file in the same folder, flushed to disk before it
/// is renamed into place, so `path` is either absent or complete.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    let written = (|| {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temporary); // best effort; the error below is what matters
    }
    Ok(written?)
}

#[cfg(test)]
mod tests;
