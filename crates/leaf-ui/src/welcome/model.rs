//! The welcome screen's small decisions, kept apart from drawing so they can be tested: how a path
//! is shortened, how the highlight moves through the recent list, and what to tell the owner when a
//! workspace won't open.

use std::path::{Component, Path};

use leaf_store::StoreError;

/// Paths longer than this many characters are shortened in the middle.
const MAX_PATH_CHARS: usize = 44;

/// `path` for display: the home folder written as `~`, and long paths shortened in the middle
/// (`~/Documents/…/Projects/Work.leaf`) so the start and the workspace's own folder stay visible.
#[must_use]
pub fn short_path(path: &Path, home: Option<&Path>) -> String {
    let (prefix, rest) = match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => ("~".to_owned(), rest.to_owned()),
        None => (String::new(), path.to_owned()),
    };
    let parts: Vec<String> = rest
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let root = if prefix.is_empty() {
        path.components()
            .take_while(|part| !matches!(part, Component::Normal(_)))
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<String>()
    } else {
        format!("{prefix}/")
    };
    let full = format!("{root}{}", parts.join("/"));
    if full.chars().count() <= MAX_PATH_CHARS || parts.len() <= 3 {
        return full;
    }
    // Keep the first folder and the last two; put an ellipsis in between.
    let first = &parts[0];
    let tail = parts[parts.len() - 2..].join("/");
    format!("{root}{first}/…/{tail}")
}

/// Where the highlight goes when Up (`down` false) or Down is pressed in a list of `len` rows,
/// wrapping at the ends. With nothing highlighted, Down starts at the top and Up at the bottom.
#[must_use]
pub fn step(selected: Option<usize>, len: usize, down: bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some(match (selected, down) {
        (None, true) => 0,
        (None, false) => len - 1,
        (Some(at), true) => (at + 1) % len,
        (Some(at), false) => (at + len - 1) % len,
    })
}

/// "Opened 5 min ago" for a recent workspace.
#[must_use]
pub fn opened_label(now: i64, at: i64) -> String {
    format!("Opened {}", leaf_editor::trashed_ago(now, at))
}

/// What to tell the owner when the workspace called `name` can't be opened.
#[must_use]
pub fn open_problem(name: &str, err: &StoreError) -> String {
    match err {
        StoreError::AlreadyOpen(_) => format!("“{name}” is already open in another Leaf window."),
        StoreError::NotAWorkspace(path) if !path.exists() => {
            format!("Leaf can't find “{name}”. It may have been moved, renamed or deleted.")
        }
        StoreError::NotAWorkspace(_) => format!("“{name}” isn't a Leaf workspace."),
        StoreError::SchemaTooNew { .. } => {
            format!("“{name}” was saved by a newer Leaf. Update Leaf to open it.")
        }
        other => format!("Leaf couldn't open “{name}”: {other}"),
    }
}

#[cfg(test)]
mod tests;
