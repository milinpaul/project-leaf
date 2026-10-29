//! Small JSON files that remember conveniences (the app settings, a workspace's sidebar state).
//!
//! Losing one is never worth stopping Leaf for, so reading never fails: a missing file gives
//! defaults, and a damaged one is moved aside (so it can be inspected and is not overwritten) and
//! replaced by defaults. Writing goes through a temporary file, so a crash leaves the old or the new
//! file, never half of one.

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::StoreError;

/// Reads `path`, or the default when it is missing, unreadable or damaged. `what` names the file in
/// log messages. A damaged file is renamed to `<name>.bad`.
pub(crate) fn load_or_default<T: DeserializeOwned + Default>(path: &Path, what: &str) -> T {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return T::default(),
        Err(err) => {
            tracing::warn!(%err, path = %path.display(), "could not read the {what}; using defaults");
            return T::default();
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(%err, path = %path.display(), "the {what} file is damaged; using defaults");
            let mut aside = path.as_os_str().to_owned();
            aside.push(".bad");
            if let Err(err) = fs::rename(path, &aside) {
                tracing::warn!(%err, "could not move the damaged {what} file aside");
            }
            T::default()
        }
    }
}

/// Writes `value` to `path` as pretty JSON, creating its folder if needed, atomically.
pub(crate) fn save<T: Serialize>(value: &T, path: &Path) -> Result<(), StoreError> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
