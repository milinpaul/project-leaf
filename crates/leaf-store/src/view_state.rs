//! A workspace's view state: how its sidebar was left on this computer (which pages were expanded).
//!
//! It lives in the workspace folder as `view.json`, next to the database but outside it, because it
//! is not part of any document: expanding a page is not an edit, so it is never undone, never
//! synced to another device and never makes the workspace "unsaved". Like the app settings it is a
//! convenience: a missing or damaged file just means everything starts collapsed. Fields this
//! version doesn't know (written by a newer Leaf) are kept on save.

use std::path::Path;

use leaf_core::PageId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{StoreError, json_file};

/// The view-state file's name inside a workspace folder.
pub const VIEW_STATE: &str = "view.json";

/// What a workspace's sidebar remembers between launches.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewState {
    /// Pages whose children the sidebar shows, as IDs. Kept as text so one damaged entry is skipped
    /// rather than losing the whole list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    expanded: Vec<String>,
    /// Fields from a newer Leaf, kept as they were.
    #[serde(flatten)]
    other: Map<String, Value>,
}

impl ViewState {
    /// Reads the view state at `path`. Never fails: a missing file gives defaults, and a damaged one
    /// is logged, moved aside to `view.json.bad` and replaced by defaults.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        json_file::load_or_default(path, "view state")
    }

    /// Writes the view state to `path` atomically.
    pub fn save(&self, path: &Path) -> Result<(), StoreError> {
        json_file::save(self, path)
    }

    /// The expanded pages; entries that are not page IDs are skipped.
    #[must_use]
    pub fn expanded(&self) -> Vec<PageId> {
        self.expanded.iter().filter_map(|id| id.parse().ok()).collect()
    }

    /// Records the expanded pages.
    pub fn set_expanded(&mut self, pages: &[PageId]) {
        self.expanded = pages.iter().map(ToString::to_string).collect();
    }
}
