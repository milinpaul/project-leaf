//! Caret movement inside one block's text.
//!
//! The model counts positions in Unicode scalars (`char`s), but what a reader sees as one character
//! can be several: an emoji family joined with zero-width joiners, a flag, a letter with an accent
//! mark, a Devanagari conjunct. The caret must never stop inside one of those, so every motion here
//! moves between grapheme cluster boundaries (Unicode UAX #29), and word jumps use UAX #29 word
//! boundaries. Inputs and results are scalar positions.

use unicode_segmentation::UnicodeSegmentation;

/// Every grapheme boundary in `text` as a scalar position, from 0 to the length.
fn boundaries(text: &str) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0).chain(text.graphemes(true).scan(0, |pos, grapheme| {
        *pos += grapheme.chars().count();
        Some(*pos)
    }))
}

/// The last grapheme boundary at or before `at`: where a caret that landed mid-character belongs.
#[must_use]
pub fn snap(text: &str, at: usize) -> usize {
    boundaries(text).take_while(|&b| b <= at).last().unwrap_or(0)
}

/// One character to the right; stays at the end.
#[must_use]
pub fn next_grapheme(text: &str, at: usize) -> usize {
    boundaries(text)
        .find(|&b| b > at)
        .unwrap_or_else(|| text.chars().count())
}

/// One character to the left; stays at the start.
#[must_use]
pub fn previous_grapheme(text: &str, at: usize) -> usize {
    boundaries(text).take_while(|&b| b < at).last().unwrap_or(0)
}

/// The words in `text` as scalar ranges. Spaces and punctuation between them are not words.
fn words(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut pos = 0;
    let mut out = Vec::new();
    for segment in text.split_word_bounds() {
        let len = segment.chars().count();
        if segment.chars().any(char::is_alphanumeric) {
            out.push(pos..pos + len);
        }
        pos += len;
    }
    out
}

/// Option/Ctrl+Right: to the end of the word the caret is in, or of the next word.
#[must_use]
pub fn word_right(text: &str, at: usize) -> usize {
    words(text)
        .into_iter()
        .find(|word| word.end > at)
        .map_or_else(|| text.chars().count(), |word| word.end)
}

/// Option/Ctrl+Left: to the start of the word the caret is in, or of the previous word.
#[must_use]
pub fn word_left(text: &str, at: usize) -> usize {
    words(text)
        .into_iter()
        .rev()
        .find(|word| word.start < at)
        .map_or(0, |word| word.start)
}

/// Home: the start of the line the caret is on. Lines end at newlines (code blocks have them);
/// wrapped lines are the layout's business (E04-S03).
#[must_use]
pub fn line_start(text: &str, at: usize) -> usize {
    text.chars()
        .take(at)
        .enumerate()
        .filter(|&(_, c)| c == '\n')
        .last()
        .map_or(0, |(i, _)| i + 1)
}

/// End: the end of the line the caret is on, before its newline.
#[must_use]
pub fn line_end(text: &str, at: usize) -> usize {
    let len = text.chars().count();
    text.chars()
        .enumerate()
        .skip(at)
        .find(|&(_, c)| c == '\n')
        .map_or(len, |(i, _)| i)
}

#[cfg(test)]
mod tests;
