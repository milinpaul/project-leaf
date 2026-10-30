//! A page's metadata as stored in the workspace doc, and how to read it.
//!
//! Keys match `docs/architecture/BLOCK_MODEL.md` ("Workspace doc"). Titles live here rather than in
//! the page's own doc so the sidebar can render without opening every page.

use leaf_core::PageId;
use loro::{LoroMap, LoroValue, ValueOrContainer};

use crate::ModelError;

/// Meta key names, shared with the page commands that write them.
pub(crate) mod keys {
    pub const ID: &str = "id";
    pub const TITLE: &str = "title";
    pub const ICON: &str = "icon";
    pub const COVER: &str = "cover";
    pub const FULL_WIDTH: &str = "full_width";
    pub const CREATED_AT: &str = "created_at";
    pub const EDITED_AT: &str = "edited_at";
    pub const TRASHED_AT: &str = "trashed_at";
}

/// What the app needs to show a page in the sidebar, switcher or header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    /// Permanent identity of the page.
    pub id: PageId,
    /// Title as typed; may be empty.
    pub title: String,
    /// A single emoji, if the page has one.
    pub icon: Option<String>,
    /// The page's cover, by name (for example `builtin:dawn`), if it has one. The model stores any
    /// non-empty name without knowing the catalogue, so a cover from a newer version survives.
    pub cover: Option<String>,
    /// Whether the page's title and blocks use the page area's whole width rather than the reading
    /// column (E18-S06). Stored only while on, so pages from before it existed read as off.
    pub full_width: bool,
    /// When the page was created, in Unix milliseconds.
    pub created_at: i64,
    /// When the page's blocks, title, icon or cover last changed, in Unix milliseconds (ADR 0059).
    /// Pages written before Leaf recorded it read as their creation time.
    pub edited_at: i64,
    /// When the page was moved to the trash, in Unix milliseconds. `None` means not in the trash.
    /// A page inside a trashed page is hidden too, even if its own value is `None`.
    pub trashed_at: Option<i64>,
}

impl PageMeta {
    /// Reads a page's metadata map. Required fields must be present; optional ones may be missing or
    /// null. Any field with the wrong type is an error, never silently ignored.
    pub(crate) fn read(id: PageId, meta: &LoroMap) -> Result<Self, ModelError> {
        let invalid = |field, reason: &str| ModelError::InvalidPageMeta {
            page: id,
            field,
            reason: reason.to_owned(),
        };
        let title = match value(meta, keys::TITLE) {
            Some(LoroValue::String(s)) => s.to_string(),
            Some(_) => return Err(invalid(keys::TITLE, "is not text")),
            None => return Err(invalid(keys::TITLE, "is missing")),
        };
        let icon = match value(meta, keys::ICON) {
            Some(LoroValue::String(s)) => Some(s.to_string()),
            Some(_) => return Err(invalid(keys::ICON, "is not text")),
            None => None,
        };
        let cover = match value(meta, keys::COVER) {
            Some(LoroValue::String(s)) => Some(s.to_string()),
            Some(_) => return Err(invalid(keys::COVER, "is not text")),
            None => None,
        };
        let full_width = match value(meta, keys::FULL_WIDTH) {
            Some(LoroValue::Bool(on)) => on,
            Some(_) => return Err(invalid(keys::FULL_WIDTH, "is not true or false")),
            None => false,
        };
        let created_at = match value(meta, keys::CREATED_AT) {
            Some(LoroValue::I64(ms)) => ms,
            Some(_) => return Err(invalid(keys::CREATED_AT, "is not a whole number")),
            None => return Err(invalid(keys::CREATED_AT, "is missing")),
        };
        let edited_at = match value(meta, keys::EDITED_AT) {
            Some(LoroValue::I64(ms)) => ms,
            Some(_) => return Err(invalid(keys::EDITED_AT, "is not a whole number")),
            None => created_at,
        };
        let trashed_at = match value(meta, keys::TRASHED_AT) {
            Some(LoroValue::I64(ms)) => Some(ms),
            Some(_) => return Err(invalid(keys::TRASHED_AT, "is not a whole number")),
            None => None,
        };
        Ok(Self {
            id,
            title,
            icon,
            cover,
            full_width,
            created_at,
            edited_at,
            trashed_at,
        })
    }
}

/// A plain value stored under `key`. Missing keys, nulls and containers all read as `None`;
/// a container where a value belongs then fails the caller's type check as "missing".
pub(crate) fn value(meta: &LoroMap, key: &str) -> Option<LoroValue> {
    match meta.get(key) {
        Some(ValueOrContainer::Value(LoroValue::Null) | ValueOrContainer::Container(_)) | None => None,
        Some(ValueOrContainer::Value(v)) => Some(v),
    }
}
