//! Looking at folders before opening or creating a workspace: what the welcome screen needs to say
//! something useful instead of a raw error.

use std::fs;
use std::path::{Path, PathBuf};

use super::DATABASE;

/// What a folder the owner picked turns out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderKind {
    /// A Leaf workspace: open it.
    Workspace,
    /// An empty folder: a workspace can be created in it.
    Empty,
    /// A folder with other things in it: not a workspace, and Leaf won't create one among them.
    Other,
    /// Nothing is there (deleted, moved, or a drive that isn't connected), or it isn't a folder.
    Missing,
}

/// Why a name can't be used for a new workspace. The messages are written for the owner.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameProblem {
    /// Nothing but spaces was typed.
    #[error("Give the workspace a name.")]
    Empty,
    /// The name has a character some systems don't allow in folder names, or starts with a dot
    /// (which hides the folder).
    #[error("Names can't contain / \\ : * ? \" < > | or start with a dot.")]
    Unsafe,
    /// The name is one Windows keeps for a device (`CON`, `NUL`, `COM1`, ...), with or without an
    /// ending. Windows can't create or open such a folder, so the workspace would be unusable there.
    #[error("“{0}” is a name Windows keeps for itself. Pick another name.")]
    Reserved(String),
    /// Something with that name is already in the chosen folder.
    #[error("There's already something called “{0}” there.")]
    Taken(String),
}

/// Characters Windows, macOS or Linux refuse (or treat specially) in folder names.
const UNSAFE: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Names Windows reserves for devices. `COM` and `LPT` take a digit (including ¹ ² ³) after them.
const DEVICES: &[&str] = &["CON", "PRN", "AUX", "NUL"];
const NUMBERED_DEVICES: &[&str] = &["COM", "LPT"];

/// The folder ending of a workspace.
const EXTENSION: &str = "leaf";

/// Looks at `path` without changing anything.
#[must_use]
pub fn inspect(path: &Path) -> FolderKind {
    if path.join(DATABASE).is_file() {
        return FolderKind::Workspace;
    }
    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                FolderKind::Empty
            } else {
                FolderKind::Other
            }
        }
        Err(_) => FolderKind::Missing,
    }
}

/// The folder a new workspace called `name` gets inside `location`: `location/name.leaf` (the ending
/// is added unless typed). Leading and trailing spaces are ignored.
pub fn new_workspace_path(location: &Path, name: &str) -> Result<PathBuf, NameProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if name.starts_with('.') || name.contains(UNSAFE) || name.chars().any(char::is_control) {
        return Err(NameProblem::Unsafe);
    }
    if is_reserved(name) {
        return Err(NameProblem::Reserved(name.to_owned()));
    }
    let folder = if Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(EXTENSION))
    {
        name.to_owned()
    } else {
        format!("{name}.{EXTENSION}")
    };
    let path = location.join(&folder);
    if path.exists() {
        return Err(NameProblem::Taken(folder));
    }
    Ok(path)
}

/// Whether Windows treats `name` as a device: the part before the first dot, without trailing
/// spaces, is a device name in any case (`nul.leaf`, `aux .leaf` and `COM¹` all count).
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_uppercase();
    if DEVICES.contains(&upper.as_str()) {
        return true;
    }
    NUMBERED_DEVICES.iter().any(|device| {
        upper.strip_prefix(device).is_some_and(|rest| {
            let mut chars = rest.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    })
}
