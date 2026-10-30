//! Page covers: the built-in catalogue, and what a page's stored cover name means.
//!
//! A page stores its cover as a name such as `builtin:dawn` (ADR 0045). The model keeps any name it is
//! given; this module decides which names this version can draw. A name it doesn't know, from a newer
//! version or a future "upload your own", stays on the page untouched and reads as
//! [`PageCover::Unknown`], which the UI shows as a plain band.

/// The prefix of every built-in cover's stored name.
const BUILTIN_PREFIX: &str = "builtin:";

/// A group of covers in the picker, shown in this order: calm plain covers first, then pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverSection {
    /// Solid colours and soft gradients (E06-S16).
    ColourAndGradient,
    /// Drawn scenes: Dawn, Forest, Ocean, Paper, Night and Travel (E06-S12).
    Illustration,
}

impl CoverSection {
    /// Every section, in the order the picker shows them.
    pub const ALL: &'static [Self] = &[Self::ColourAndGradient, Self::Illustration];

    /// The section's heading in the picker.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ColourAndGradient => "Colour & gradient",
            Self::Illustration => "Illustrations",
        }
    }

    /// The section's covers, in picker order.
    pub fn covers(self) -> impl Iterator<Item = BuiltinCover> {
        BuiltinCover::ALL
            .iter()
            .copied()
            .filter(move |cover| cover.section() == self)
    }
}

/// A cover that ships with Leaf. Each one's [`name`](Self::name) is stored in pages, so it must never
/// change once released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinCover {
    /// Soft grey-green.
    Sage,
    /// Pale blue-grey.
    Mist,
    /// Warm beige.
    Sand,
    /// Faded pink.
    Blush,
    /// Pale lilac.
    Lavender,
    /// Pale mint.
    Seafoam,
    /// Deep blue-grey, calm in dark mode.
    Slate,
    /// Deep green, calm in dark mode.
    Pine,
    /// Peach to pink to lilac.
    Sunrise,
    /// Mint to sky blue.
    Lagoon,
    /// Cream to sage.
    Meadow,
    /// Ice white to pale teal.
    Glacier,
    /// Deep blue to violet to rose.
    Twilight,
    /// Teal and violet glows on night blue.
    Aurora,
    /// Pink, peach and lilac glows on cream.
    Petal,
    /// A peach glow in blue-grey mist.
    Haze,
    /// Warm peach-to-rose: writing and journals.
    Dawn,
    /// Misty layers of pine trees: personal and nature.
    Forest,
    /// Calm water seen from above, with ripples: work and projects.
    Ocean,
    /// Soft off-white with a faint texture: notes and research.
    Paper,
    /// Indigo-to-violet with stars: ideas and creative work.
    Night,
    /// A map with a flight route, pins and a compass: trips and travel plans.
    Travel,
}

impl BuiltinCover {
    /// Every built-in cover, in the order the picker shows them.
    pub const ALL: &'static [Self] = &[
        Self::Sage,
        Self::Mist,
        Self::Sand,
        Self::Blush,
        Self::Lavender,
        Self::Seafoam,
        Self::Slate,
        Self::Pine,
        Self::Sunrise,
        Self::Lagoon,
        Self::Meadow,
        Self::Glacier,
        Self::Twilight,
        Self::Aurora,
        Self::Petal,
        Self::Haze,
        Self::Dawn,
        Self::Forest,
        Self::Ocean,
        Self::Paper,
        Self::Night,
        Self::Travel,
    ];

    /// The stable name, used in the stored value and the cover's file name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sage => "sage",
            Self::Mist => "mist",
            Self::Sand => "sand",
            Self::Blush => "blush",
            Self::Lavender => "lavender",
            Self::Seafoam => "seafoam",
            Self::Slate => "slate",
            Self::Pine => "pine",
            Self::Sunrise => "sunrise",
            Self::Lagoon => "lagoon",
            Self::Meadow => "meadow",
            Self::Glacier => "glacier",
            Self::Twilight => "twilight",
            Self::Aurora => "aurora",
            Self::Petal => "petal",
            Self::Haze => "haze",
            Self::Dawn => "dawn",
            Self::Forest => "forest",
            Self::Ocean => "ocean",
            Self::Paper => "paper",
            Self::Night => "night",
            Self::Travel => "travel",
        }
    }

    /// The name shown to the owner in the picker.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sage => "Sage",
            Self::Mist => "Mist",
            Self::Sand => "Sand",
            Self::Blush => "Blush",
            Self::Lavender => "Lavender",
            Self::Seafoam => "Seafoam",
            Self::Slate => "Slate",
            Self::Pine => "Pine",
            Self::Sunrise => "Sunrise",
            Self::Lagoon => "Lagoon",
            Self::Meadow => "Meadow",
            Self::Glacier => "Glacier",
            Self::Twilight => "Twilight",
            Self::Aurora => "Aurora",
            Self::Petal => "Petal",
            Self::Haze => "Haze",
            Self::Dawn => "Dawn",
            Self::Forest => "Forest",
            Self::Ocean => "Ocean",
            Self::Paper => "Paper",
            Self::Night => "Night",
            Self::Travel => "Travel",
        }
    }

    /// The picker section the cover belongs in.
    #[must_use]
    pub const fn section(self) -> CoverSection {
        match self {
            Self::Dawn | Self::Forest | Self::Ocean | Self::Paper | Self::Night | Self::Travel => {
                CoverSection::Illustration
            }
            _ => CoverSection::ColourAndGradient,
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
    use super::{BuiltinCover, CoverSection, PageCover};

    #[test]
    fn built_in_covers_round_trip_through_their_stored_name() {
        for &cover in BuiltinCover::ALL {
            assert_eq!(PageCover::parse(&cover.stored()), PageCover::Builtin(cover));
            assert!(
                cover.label().starts_with(char::is_uppercase),
                "labels read as names"
            );
        }
        let stored: Vec<String> = BuiltinCover::ALL.iter().map(|c| c.stored()).collect();
        assert_eq!(
            stored,
            [
                "builtin:sage",
                "builtin:mist",
                "builtin:sand",
                "builtin:blush",
                "builtin:lavender",
                "builtin:seafoam",
                "builtin:slate",
                "builtin:pine",
                "builtin:sunrise",
                "builtin:lagoon",
                "builtin:meadow",
                "builtin:glacier",
                "builtin:twilight",
                "builtin:aurora",
                "builtin:petal",
                "builtin:haze",
                "builtin:dawn",
                "builtin:forest",
                "builtin:ocean",
                "builtin:paper",
                "builtin:night",
                "builtin:travel",
            ],
            "stored names never change once shipped"
        );
    }

    #[test]
    fn names_this_version_does_not_know_are_kept_as_they_are() {
        for stored in [
            "builtin:aurora",
            "builtin:Forest",
            "image:0123abcd",
            "dawn",
            "builtin:",
        ] {
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

    #[test]
    fn sections_split_the_covers_in_picker_order() {
        let mut shown = Vec::new();
        for &section in CoverSection::ALL {
            assert_ne!(section.label(), "");
            shown.extend(section.covers());
        }
        assert_eq!(
            shown,
            BuiltinCover::ALL,
            "each cover in exactly one section, in order"
        );
        let pictures: Vec<_> = CoverSection::Illustration
            .covers()
            .map(BuiltinCover::name)
            .collect();
        assert_eq!(pictures, ["dawn", "forest", "ocean", "paper", "night", "travel"]);
        assert_eq!(CoverSection::ColourAndGradient.covers().count(), 16);
    }
}
