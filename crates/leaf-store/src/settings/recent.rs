//! The workspaces opened recently: the last one is reopened at launch, and the welcome screen lists
//! them all (E06-S09).
//!
//! A workspace that has gone missing (deleted, renamed, on an unplugged drive) stays in the list,
//! flagged, so the owner sees what happened and can remove it; it is never dropped silently.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::AppSettings;

/// The most workspaces the list remembers.
pub const MAX_RECENT_WORKSPACES: usize = 10;

/// One remembered workspace, as stored in the settings file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Saved {
    /// The workspace folder. Text, so the file stays readable; folders whose names aren't valid
    /// Unicode are not remembered.
    path: String,
    /// When it was last opened, in Unix milliseconds.
    opened_at: i64,
}

/// A recently opened workspace, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentWorkspace {
    /// The workspace folder.
    pub path: PathBuf,
    /// Its name: the folder name without the `.leaf` ending.
    pub name: String,
    /// When it was last opened, in Unix milliseconds.
    pub opened_at: i64,
    /// Whether the folder is no longer there (deleted, moved, or on a drive that isn't connected).
    pub missing: bool,
}

impl AppSettings {
    /// Records that the workspace at `path` was opened at `now` (Unix milliseconds): it moves to the
    /// front of the recent list, appears there once, and the oldest falls off past
    /// [`MAX_RECENT_WORKSPACES`].
    pub fn remember_workspace(&mut self, path: &Path, now: i64) {
        let path = normalized(path);
        let Some(text) = path.to_str() else {
            return;
        };
        self.recent_workspaces
            .retain(|saved| normalized(Path::new(&saved.path)) != path);
        self.recent_workspaces.insert(
            0,
            Saved {
                path: text.to_owned(),
                opened_at: now,
            },
        );
        self.recent_workspaces.truncate(MAX_RECENT_WORKSPACES);
    }

    /// The workspace opened most recently, if any, whether or not it is still there.
    #[must_use]
    pub fn last_workspace(&self) -> Option<PathBuf> {
        self.recent_workspaces
            .first()
            .map(|saved| PathBuf::from(&saved.path))
    }

    /// The recent workspaces, most recent first, each flagged if `exists` says its folder is gone.
    /// (`exists` is `Path::exists` in the app; tests pass their own.)
    pub fn recent_workspaces(&self, exists: impl Fn(&Path) -> bool) -> Vec<RecentWorkspace> {
        self.recent_workspaces
            .iter()
            .map(|saved| {
                let path = PathBuf::from(&saved.path);
                RecentWorkspace {
                    name: workspace_name(&path),
                    missing: !exists(&path),
                    opened_at: saved.opened_at,
                    path,
                }
            })
            .collect()
    }

    /// "Remove from list": forgets the workspace at `path`. Its folder is not touched.
    pub fn forget_workspace(&mut self, path: &Path) {
        let path = normalized(path);
        self.recent_workspaces
            .retain(|saved| normalized(Path::new(&saved.path)) != path);
    }
}

/// The same folder written two ways (`./a/../w.leaf`, a symlink) counts once: the real path when the
/// folder exists, else the path as given.
fn normalized(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

/// The folder name without `.leaf`, as the sidebar header shows it.
fn workspace_name(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

/// Reads the saved list, skipping entries this version can't read instead of failing the file.
pub(super) fn readable_entries<'de, D: Deserializer<'de>>(reader: D) -> Result<Vec<Saved>, D::Error> {
    // Read the whole value first, so something that isn't a list is skipped too (not half-read).
    let values: Vec<Value> = serde_json::from_value(Value::deserialize(reader)?).unwrap_or_default();
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}
