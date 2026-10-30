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

/// The picker's state: the search text, the sections it leaves and the highlighted emoji.
#[derive(Debug, Clone)]
pub struct EmojiPicker {
    query: String,
    sections: Vec<EmojiSection>,
    /// The highlighted emoji, as (section, position in it): what Enter picks and arrows move. The
    /// first emoji until the arrows or the mouse move it; `None` only when nothing is shown.
    highlighted: Option<(usize, usize)>,
}

/// An arrow key in the picker's grid (E17-S04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMove {
    /// The emoji before, into the previous section from a section's first.
    Left,
    /// The emoji after, into the next section from a section's last.
    Right,
    /// The row above, in the same column (the last row of the previous section from a first row).
    Up,
    /// The row below, in the same column (the first row of the next section from a last row).
    Down,
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
        let sections = all_sections();
        Self {
            query: String::new(),
            highlighted: first_of(&sections),
            sections,
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
        self.highlighted = first_of(&self.sections);
    }

    /// The sections to draw.
    #[must_use]
    pub fn sections(&self) -> &[EmojiSection] {
        &self.sections
    }

    /// The first emoji shown.
    #[must_use]
    pub fn first(&self) -> Option<EmojiChoice> {
        self.sections
            .first()
            .and_then(|section| section.emojis.first().copied())
    }

    /// Where the highlighted emoji is, as (section, position in it).
    #[must_use]
    pub fn highlighted(&self) -> Option<(usize, usize)> {
        self.highlighted
    }

    /// The highlighted emoji, which Enter picks: the best match as you type, or where the arrows or the
    /// mouse moved it.
    #[must_use]
    pub fn highlighted_choice(&self) -> Option<EmojiChoice> {
        let (section, at) = self.highlighted?;
        self.sections.get(section)?.emojis.get(at).copied()
    }

    /// The mouse over an emoji: highlights it. Positions outside the grid are ignored.
    pub fn highlight(&mut self, section: usize, at: usize) {
        if self.sections.get(section).is_some_and(|s| at < s.emojis.len()) {
            self.highlighted = Some((section, at));
        }
    }

    /// Moves the highlight with an arrow key over a grid `columns` wide. At the first or last emoji it
    /// stays put.
    pub fn move_highlight(&mut self, step: GridMove, columns: usize) {
        let Some((section, at)) = self.highlighted else {
            return;
        };
        let columns = columns.max(1);
        let len = |s: usize| self.sections.get(s).map_or(0, |section| section.emojis.len());
        let last_row_start = |s: usize| len(s).saturating_sub(1) / columns * columns;
        let target = match step {
            GridMove::Left if at > 0 => Some((section, at - 1)),
            GridMove::Left if section > 0 => Some((section - 1, len(section - 1).saturating_sub(1))),
            GridMove::Right if at + 1 < len(section) => Some((section, at + 1)),
            GridMove::Right if section + 1 < self.sections.len() => Some((section + 1, 0)),
            GridMove::Up if at >= columns => Some((section, at - columns)),
            GridMove::Up if section > 0 => {
                let above = section - 1;
                Some((
                    above,
                    (last_row_start(above) + at % columns).min(len(above).saturating_sub(1)),
                ))
            }
            GridMove::Down if at / columns < last_row_start(section) / columns => {
                Some((section, (at + columns).min(len(section) - 1)))
            }
            GridMove::Down if section + 1 < self.sections.len() => {
                let below = section + 1;
                Some((below, (at % columns).min(len(below).saturating_sub(1))))
            }
            _ => None,
        };
        if let Some(target) = target {
            self.highlighted = Some(target);
        }
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

/// The first emoji of `sections`, if any.
fn first_of(sections: &[EmojiSection]) -> Option<(usize, usize)> {
    sections
        .iter()
        .position(|section| !section.emojis.is_empty())
        .map(|section| (section, 0))
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
