//! Permanent identities for pages, blocks and databases (with their columns and select options).
//!
//! Both are UUID v7: globally unique, so they survive copy and paste between pages and later sync
//! between devices, and time-ordered, so sorting by ID roughly sorts by creation time. Blocks keep
//! theirs in their own meta rather than using Loro's `TreeID`, which changes on undo (ADR-006).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CoreError;

/// Defines a UUID v7 newtype with text and serde round-trips, so every ID type behaves the same.
macro_rules! uuid_id {
    ($(#[$doc:meta])* $name:ident, $kind:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Mints a fresh ID. Never reuse one: pasted and duplicated items get new IDs too.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Wraps an existing UUID, for example one read back from storage.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID, for storage layers that store UUIDs natively.
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl FromStr for $name {
            type Err = CoreError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self).map_err(|e| CoreError::InvalidId {
                    kind: $kind,
                    input: s.to_owned(),
                    reason: e.to_string(),
                })
            }
        }
    };
}

uuid_id!(
    /// Identifies a page across the whole workspace, forever. Stored in the page's meta in the
    /// workspace doc and used as the key of the page's own document.
    PageId,
    "page"
);

uuid_id!(
    /// Identifies a block within its page, forever: survives moves, merges, undo and redo.
    /// Stored as the `id` field of the block's meta. A block reference is `(PageId, BlockId)`.
    BlockId,
    "block"
);

uuid_id!(
    /// Identifies a database across the whole workspace, forever: the key of its own document (ADR-066).
    /// A database's rows are pages, so they use [`PageId`].
    DatabaseId,
    "database"
);

uuid_id!(
    /// Identifies a column (property) within its database, forever: survives renames, moves and type
    /// changes, so cells and the view refer to it by this rather than by name or position.
    ColumnId,
    "column"
);

uuid_id!(
    /// Identifies one option of a select or multi-select column, forever: cells hold this, so renaming
    /// or recolouring an option is one edit.
    OptionId,
    "option"
);

#[cfg(test)]
mod tests {
    use super::{BlockId, ColumnId, DatabaseId, OptionId, PageId};
    use crate::CoreError;

    #[test]
    fn new_ids_are_unique_v7_and_time_ordered() {
        let (a, b) = (PageId::new(), PageId::new());
        assert_ne!(a, b);
        assert_eq!(a.as_uuid().get_version_num(), 7);
        assert!(a < b, "a later v7 id sorts after an earlier one");
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = BlockId::new();
        let text = id.to_string();
        assert_eq!(text.len(), 36, "hyphenated form: {text}");
        assert_eq!(text.parse::<BlockId>(), Ok(id));
    }

    #[test]
    fn parsing_accepts_uppercase_and_rejects_garbage() {
        let id = PageId::new();
        assert_eq!(id.to_string().to_uppercase().parse::<PageId>(), Ok(id));
        let err = "not-a-uuid".parse::<PageId>().unwrap_err();
        assert!(matches!(&err, CoreError::InvalidId { kind: "page", input, .. } if input == "not-a-uuid"));
        assert!(
            err.to_string().starts_with("invalid page id \"not-a-uuid\""),
            "{err}"
        );
    }

    #[test]
    fn serde_round_trip_uses_the_text_form() {
        let id = PageId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<PageId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<PageId>("\"nope\"").is_err());
    }

    #[test]
    fn database_ids_round_trip_and_name_their_kind() {
        let (database, column, option) = (DatabaseId::new(), ColumnId::new(), OptionId::new());
        assert_eq!(database.to_string().parse::<DatabaseId>(), Ok(database));
        assert_eq!(column.to_string().parse::<ColumnId>(), Ok(column));
        assert_eq!(option.to_string().parse::<OptionId>(), Ok(option));
        assert!(matches!(
            "x".parse::<ColumnId>(),
            Err(CoreError::InvalidId { kind: "column", .. })
        ));
    }

    #[test]
    fn uuid_conversions_and_default() {
        let uuid = uuid::Uuid::now_v7();
        assert_eq!(BlockId::from_uuid(uuid).as_uuid(), &uuid);
        assert_ne!(BlockId::default(), BlockId::default());
    }
}
