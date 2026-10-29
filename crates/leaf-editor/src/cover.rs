//! Page covers: the built-in catalogue, and what a page's stored cover name means.
//!
//! A page stores its cover as a name such as `builtin:dawn` (ADR 0045). The model keeps any name it is
//! given; this module decides which names this version can draw. A name it doesn't know, from a newer
//! version or a future "upload your own", stays on the page untouched and reads as
//! [`PageCover::Unknown`], which the UI shows as a plain band.

/// The prefix of every built-in cover's stored name.
const BUILTIN_PREFIX: &str = "builtin:";

/// A cover that ships with Leaf. Each one's [`name`](Self::name) is stored in pages, so it must never
/// change once released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinCover {
    /// Warm peach-to-rose: writing and journals.
    Dawn,
}

impl BuiltinCover {
    /// Every built-in cover, in the order the picker shows them.
    pub const ALL: &'static [Self] = &[Self::Dawn];

    /// The stable name, used in the stored value and the cover's file name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dawn => "dawn",
        }
    }

    /// The name shown to the owner in the picker.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Dawn => "Dawn",
        }
    }

    /// The value stored on a page that uses this cover, such as `builtin:dawn`.
    #[must_use]
    pub fn stored(self) -> String {
        format!("{BUILTIN_PREFIX}{}", self.name())
    }
}

/// A page's cover as this version understands it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageCover {
    /// One of the covers this version ships.
    Builtin(BuiltinCover),
    /// A name this version doesn't know, kept as stored.
    Unknown(String),
}

impl PageCover {
    /// Reads a stored cover name.
    #[must_use]
    pub fn parse(stored: &str) -> Self {
        stored
            .strip_prefix(BUILTIN_PREFIX)
            .and_then(|name| BuiltinCover::ALL.iter().copied().find(|c| c.name() == name))
            .map_or_else(|| Self::Unknown(stored.to_owned()), Self::Builtin)
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinCover, PageCover};

    #[test]
    fn built_in_covers_round_trip_through_their_stored_name() {
        for &cover in BuiltinCover::ALL {
            assert_eq!(PageCover::parse(&cover.stored()), PageCover::Builtin(cover));
            assert!(
                cover.label().starts_with(char::is_uppercase),
                "labels read as names"
            );
        }
        assert_eq!(
            BuiltinCover::Dawn.stored(),
            "builtin:dawn",
            "stored names never change"
        );
    }

    #[test]
    fn names_this_version_does_not_know_are_kept_as_they_are() {
        for stored in ["builtin:aurora", "image:0123abcd", "dawn", "builtin:"] {
            assert_eq!(PageCover::parse(stored), PageCover::Unknown(stored.to_owned()));
        }
    }

    #[test]
    fn stored_names_are_unique() {
        let mut names: Vec<_> = BuiltinCover::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), BuiltinCover::ALL.len());
    }
}
