//! Fuzzy matching of a typed query against a short text such as a page title, for the quick switcher.
//!
//! Each word of the query must match on its own, in any order, so "notes design" finds "Design notes".
//! A word scores best as the whole title, then as its start, then as the start of a word inside it,
//! then anywhere inside it, and last as letters in order with others between them ("dsgn"). Matching
//! ignores case. It returns where the letters matched, so the result can show them in bold.

use std::ops::Range;

/// A successful match: how good it is (higher is better) and which parts of the text matched, as
/// byte ranges in order, never overlapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Higher is better; only meaningful compared with other matches for the same query.
    pub score: u32,
    /// The matched parts of the text, as byte ranges.
    pub ranges: Vec<Range<usize>>,
}

/// Scores for each kind of match, far enough apart that a better kind always wins.
const WHOLE: u32 = 10_000;
const PREFIX: u32 = 8_000;
const WORD_START: u32 = 6_000;
const INSIDE: u32 = 4_000;
const IN_ORDER: u32 = 1_000;

/// Matches `query` against `text`. `None` when some word of the query doesn't match; an empty query
/// matches everything with score 0.
#[must_use]
pub fn fuzzy_match(text: &str, query: &str) -> Option<FuzzyMatch> {
    Lowered::new(text).matches(&query_words(query))
}

/// A query split into lowercase words, prepared once for matching against many texts.
pub(crate) fn query_words(query: &str) -> Vec<Vec<char>> {
    query
        .split_whitespace()
        .map(|word| word.chars().map(lower).collect())
        .collect()
}

/// The best match of one lowercase query word in the lowercase `text`, with the matched char indices.
fn match_word(text: &[char], word: &[char]) -> Option<(u32, Vec<usize>)> {
    if text == word {
        return Some((WHOLE, (0..word.len()).collect()));
    }
    let starts: Vec<usize> = match word.first() {
        Some(&first) => (0..text.len())
            .filter(|&at| text[at] == first && text[at..].starts_with(word))
            .collect(),
        None => Vec::new(),
    };
    let span = |at: usize| (at..at + word.len()).collect::<Vec<_>>();
    // Earlier and shorter is better within each kind; the adjustments never reach the next kind.
    let closeness = |at: usize| 999u32.saturating_sub(u32::try_from(at).unwrap_or(u32::MAX));
    if starts.first() == Some(&0) {
        let shorter = 999u32.saturating_sub(u32::try_from(text.len()).unwrap_or(u32::MAX));
        return Some((PREFIX + shorter, span(0)));
    }
    if let Some(&at) = starts.iter().find(|&&at| is_word_start(text, at)) {
        return Some((WORD_START + closeness(at), span(at)));
    }
    if let Some(&at) = starts.first() {
        return Some((INSIDE + closeness(at), span(at)));
    }
    in_order(text, word).map(|(closeness, positions)| (IN_ORDER + closeness, positions))
}

/// Whether the word's letters appear in order in `text`; if so, a closeness score (higher the fewer
/// letters lie between them, with a bonus for each matched letter that starts a word) and where they are.
fn in_order(text: &[char], word: &[char]) -> Option<(u32, Vec<usize>)> {
    let mut positions = Vec::with_capacity(word.len());
    let mut at = 0;
    for &wanted in word {
        let found = (at..text.len()).find(|&i| text[i] == wanted)?;
        positions.push(found);
        at = found + 1;
    }
    let first = positions.first().copied().unwrap_or(0);
    let gaps = positions.last().copied().unwrap_or(0) - first + 1 - positions.len();
    let starts = positions.iter().filter(|&&i| is_word_start(text, i)).count();
    let score = 500u32
        .saturating_sub(u32::try_from(gaps * 10 + first).unwrap_or(u32::MAX))
        .saturating_add(u32::try_from(starts * 50).unwrap_or(u32::MAX))
        .min(999);
    Some((score, positions))
}

/// Whether the letter at `at` begins a word: the first letter, or one after a space or punctuation.
fn is_word_start(text: &[char], at: usize) -> bool {
    at == 0 || text.get(at - 1).is_some_and(|c| !c.is_alphanumeric())
}

/// A character's lowercase form, one character for one, so positions line up with the original.
fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// A text's characters in lowercase, with each one's byte range in the original: prepared once, so
/// matching it against each keystroke's query doesn't redo the work.
#[derive(Debug, Clone)]
pub(crate) struct Lowered {
    chars: Vec<char>,
    bytes: Vec<Range<usize>>,
}

impl Lowered {
    pub(crate) fn new(text: &str) -> Self {
        let (chars, bytes) = text
            .char_indices()
            .map(|(at, c)| (lower(c), at..at + c.len_utf8()))
            .unzip();
        Self { chars, bytes }
    }

    /// Matches the prepared query `words` (from [`query_words`]); see [`fuzzy_match`].
    pub(crate) fn matches(&self, words: &[Vec<char>]) -> Option<FuzzyMatch> {
        let mut score = 0;
        let mut matched: Vec<usize> = Vec::new(); // char indices
        for word in words {
            let (word_score, positions) = match_word(&self.chars, word)?;
            score += word_score;
            matched.extend(positions);
        }
        matched.sort_unstable();
        matched.dedup();
        Some(FuzzyMatch {
            score,
            ranges: self.byte_ranges(&matched),
        })
    }

    /// Sorted char indices as byte ranges, neighbours joined.
    fn byte_ranges(&self, indices: &[usize]) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = Vec::new();
        for bytes in indices.iter().filter_map(|&i| self.bytes.get(i)) {
            match ranges.last_mut() {
                Some(last) if last.end == bytes.start => last.end = bytes.end,
                _ => ranges.push(bytes.clone()),
            }
        }
        ranges
    }
}

#[cfg(test)]
mod tests;
