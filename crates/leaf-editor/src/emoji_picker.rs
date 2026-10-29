//! The page icon picker: every emoji in sections (Smileys & people, Animals & nature, …), narrowed by a
//! search over names and shortcodes as the owner types.
//!
//! The catalogue comes from the `emojis` crate (Unicode's emoji list and GitHub's shortcodes). Only emoji
//! from Unicode 13 or older are offered, so the fonts on every supported OS can draw them; skin-tone
//! variants are left out to keep the grid calm.

use emojis::{Emoji, Group, UnicodeVersion};

/// The newest Unicode version whose emoji the picker offers (2020): drawn by Windows 10, macOS 11 and
/// the Noto Color Emoji in current Linux distributions.
const NEWEST_UNICODE: UnicodeVersion = UnicodeVersion::new(13, 0);

/// The section title for search results.
pub const RESULTS_TITLE: &str = "Results";

/// One emoji the picker offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmojiChoice {
    /// The emoji itself, as stored in a page's icon.
    pub emoji: &'static str,
    /// Its Unicode name ("grinning face"), for tooltips and search.
    pub name: &'static str,
}

/// A titled run of emoji in the picker's grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiSection {
    /// "Smileys & emotion", or [`RESULTS_TITLE`] while searching.
    pub title: &'static str,
    /// The emoji, in catalogue order (or best match first while searching).
    pub emojis: Vec<EmojiChoice>,
}

/// The picker's state: the search text and the sections it leaves.
#[derive(Debug, Clone)]
pub struct EmojiPicker {
    query: String,
    sections: Vec<EmojiSection>,
}

impl Default for EmojiPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl EmojiPicker {
    /// The picker with nothing typed: every offered emoji, by group.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            sections: all_sections(),
        }
    }

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Narrows the picker to emoji whose name, shortcode or group has a word starting with each typed word, best
    /// match first, in one [`RESULTS_TITLE`] section (none when nothing matches). An empty query shows
    /// every section again.
    pub fn set_query(&mut self, query: &str) {
        if self.query == query {
            return;
        }
        query.clone_into(&mut self.query);
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.sections = if words.is_empty() {
            all_sections()
        } else {
            let mut found: Vec<(u32, usize, EmojiChoice)> = offered()
                .enumerate()
                .filter_map(|(order, emoji)| score(emoji, &words).map(|s| (s, order, choice(emoji))))
                .collect();
            found.sort_by_key(|&(score, order, _)| (std::cmp::Reverse(score), order));
            let emojis: Vec<EmojiChoice> = found.into_iter().map(|(_, _, choice)| choice).collect();
            if emojis.is_empty() {
                Vec::new()
            } else {
                vec![EmojiSection {
                    title: RESULTS_TITLE,
                    emojis,
                }]
            }
        };
    }

    /// The sections to draw.
    #[must_use]
    pub fn sections(&self) -> &[EmojiSection] {
        &self.sections
    }

    /// The first emoji shown, which Enter picks.
    #[must_use]
    pub fn first(&self) -> Option<EmojiChoice> {
        self.sections
            .first()
            .and_then(|section| section.emojis.first().copied())
    }

    /// A pleasant emoji for the Random button, chosen by `seed` (the app passes something that changes,
    /// such as the time; the model never reads the clock). Picks from the friendlier groups, not flags or
    /// symbols.
    #[must_use]
    pub fn random(seed: u64) -> EmojiChoice {
        let pool: Vec<&'static Emoji> = offered()
            .filter(|emoji| {
                matches!(
                    emoji.group(),
                    Group::SmileysAndEmotion
                        | Group::AnimalsAndNature
                        | Group::FoodAndDrink
                        | Group::TravelAndPlaces
                        | Group::Activities
                        | Group::Objects
                )
            })
            .collect();
        let index = usize::try_from(seed % u64::try_from(pool.len()).unwrap_or(1)).unwrap_or(0);
        pool.get(index).copied().map_or(
            EmojiChoice {
                emoji: "📄",
                name: "page facing up",
            },
            choice,
        )
    }
}

/// Every emoji the picker offers, in Unicode's order.
fn offered() -> impl Iterator<Item = &'static Emoji> {
    emojis::iter().filter(|emoji| {
        emoji.unicode_version() <= NEWEST_UNICODE
            && emoji
                .skin_tone()
                .is_none_or(|tone| tone == emojis::SkinTone::Default)
    })
}

fn choice(emoji: &'static Emoji) -> EmojiChoice {
    EmojiChoice {
        emoji: emoji.as_str(),
        name: emoji.name(),
    }
}

/// The offered emoji grouped into titled sections, in Unicode's group order.
fn all_sections() -> Vec<EmojiSection> {
    Group::iter()
        .map(|group| EmojiSection {
            title: group_title(group),
            emojis: offered()
                .filter(|emoji| emoji.group() == group)
                .map(choice)
                .collect(),
        })
        .filter(|section| !section.emojis.is_empty())
        .collect()
}

fn group_title(group: Group) -> &'static str {
    match group {
        Group::SmileysAndEmotion => "Smileys & emotion",
        Group::PeopleAndBody => "People & body",
        Group::AnimalsAndNature => "Animals & nature",
        Group::FoodAndDrink => "Food & drink",
        Group::TravelAndPlaces => "Travel & places",
        Group::Activities => "Activities",
        Group::Objects => "Objects",
        Group::Symbols => "Symbols",
        Group::Flags => "Flags",
    }
}

/// How well `words` (lowercase) match `emoji`; `None` unless every word starts a word of its name, of
/// one of its shortcodes or of its group's title. The whole name scores best, then a name that starts with the query, then
/// earlier matches.
fn score(emoji: &Emoji, words: &[String]) -> Option<u32> {
    let name = emoji.name().to_lowercase();
    let group = group_title(emoji.group()).to_lowercase();
    // Name words first, then shortcodes, then the group ("food", "animals"), so earlier counts as better.
    let mut vocabulary: Vec<&str> = name.split([' ', '-', ':', ',']).collect();
    vocabulary.extend(emoji.shortcodes().flat_map(|code| code.split('_')));
    vocabulary.extend(group.split([' ', '&']).filter(|word| !word.is_empty()));
    let mut first_hit = usize::MAX;
    for word in words {
        let at = vocabulary
            .iter()
            .position(|known| known.starts_with(word.as_str()))?;
        first_hit = first_hit.min(at);
    }
    let query = words.join(" ");
    let score = if name == query {
        3_000
    } else if name.starts_with(&query) {
        2_000
    } else {
        1_000u32.saturating_sub(u32::try_from(first_hit).unwrap_or(u32::MAX))
    };
    Some(score)
}

#[cfg(test)]
mod tests;
