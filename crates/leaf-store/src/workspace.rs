//! A workspace folder on disk: creating it, opening it, and making sure only one app has it open.

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};

use crate::{Store, StoreError};

mod folders;

pub use folders::{FolderKind, NameProblem, inspect, new_workspace_path};

/// The database file inside a workspace folder.
pub const DATABASE: &str = "workspace.sqlite";
/// Folder for image files, named by content hash.
pub const ASSETS: &str = "assets";
/// Folder for the search index (rebuildable from the documents).
pub const SEARCH: &str = "search";
/// The lock file held while the workspace is open.
pub const LOCK: &str = "leaf.lock";

/// An open workspace folder: its database plus the lock that keeps other app instances out.
///
/// The lock is an operating-system file lock, so the OS drops it when the app quits or crashes; a
/// leftover `leaf.lock` file never blocks the owner. It is released when the `Workspace` is dropped.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    store: Store,
    _lock: File,
}

impl Workspace {
    /// Creates a new workspace at `path` (usually ending in `.leaf`).
    ///
    /// `path` must not exist yet, or be an empty folder (for example one just made in a save
    /// dialog). Anything else fails with [`StoreError::AlreadyExists`].
    pub fn create(path: &Path) -> Result<Self, StoreError> {
        match fs::read_dir(path) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err(StoreError::AlreadyExists(path.to_owned()));
                }
            }
            Err(_) if path.exists() => return Err(StoreError::AlreadyExists(path.to_owned())),
            Err(_) => fs::create_dir_all(path)?,
        }
        Self::open_folder(path)
    }

    /// Opens the existing workspace at `path`, upgrading its database if it is from an older Leaf.
    ///
    /// Fails with [`StoreError::NotAWorkspace`] unless `path` is a folder with a `workspace.sqlite`,
    /// and with [`StoreError::AlreadyOpen`] while another Leaf has it open.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if !path.join(DATABASE).is_file() {
            return Err(StoreError::NotAWorkspace(path.to_owned()));
        }
        Self::open_folder(path)
    }

    /// Opens the workspace at `path`, or creates it there if nothing exists yet: what the app does
    /// at launch. Anything else at `path` (a file, a folder of other things) is an error, not
    /// overwritten.
    pub fn open_or_create(path: &Path) -> Result<Self, StoreError> {
        if path.exists() {
            match Self::open(path) {
                Err(StoreError::NotAWorkspace(_)) => Self::create(path),
                other => other,
            }
        } else {
            Self::create(path)
        }
    }

    /// The workspace folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where this workspace keeps its [`ViewState`](crate::ViewState) (`view.json`).
    #[must_use]
    pub fn view_state_path(&self) -> PathBuf {
        self.root.join(crate::VIEW_STATE)
    }

    /// The workspace's database.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The workspace's database, for saving.
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// A second connection to this workspace's database, for a reader on another thread such as
    /// the search indexer. Write-ahead logging lets it read while autosave writes; it never needs
    /// the lock, which this workspace already holds.
    pub fn reader(&self) -> Result<Store, StoreError> {
        Store::open(&self.root.join(DATABASE))
    }

    /// Takes the lock first, so a second app can't touch the database at all, then makes sure the
    /// subfolders exist (a user may have deleted an empty one) and opens the database.
    fn open_folder(path: &Path) -> Result<Self, StoreError> {
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.join(LOCK))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(StoreError::AlreadyOpen(path.to_owned())),
            Err(TryLockError::Error(err)) => return Err(err.into()),
        }
        fs::create_dir_all(path.join(ASSETS))?;
        fs::create_dir_all(path.join(SEARCH))?;
        let store = Store::open(&path.join(DATABASE))?;
        Ok(Self {
            root: path.to_owned(),
            store,
            _lock: lock,
        })
    }
}
