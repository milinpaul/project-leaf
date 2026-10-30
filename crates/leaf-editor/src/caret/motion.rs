//! Caret movement inside one block's text.
//!
//! The model counts positions in Unicode scalars (`char`s), but what a reader sees as one character
//! can be several: an emoji family joined with zero-width joiners, a flag, a letter with an accent
//! mark, a Devanagari conjunct. The caret must never stop inside one of those, so every motion here
//! moves between grapheme cluster boundaries (Unicode UAX #29), and word jumps use UAX #29 word
//! boundaries. Inputs and results are scalar positions.

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

// Every motion looks only at the text around the caret (E17-S08 follow-up, E17-S09): segmenting a
// whole block from its start on each key press made arrows lag in a 38,000-character paragraph.

/// The byte offset of scalar position `at`, or the text's length past its end.
fn byte_of(text: &str, at: usize) -> usize {
    text.char_indices().nth(at).map_or(text.len(), |(byte, _)| byte)
}

/// A grapheme cursor at `at` over the whole of `text`, which it may read around `at` but no further
/// than it needs. Returns it with `at` clamped to the text and its byte offset.
fn cursor(text: &str, at: usize) -> (GraphemeCursor, usize, usize) {
    let byte = byte_of(text, at);
    let at = if byte == text.len() {
        text.chars().count()
    } else {
        at
    };
    (GraphemeCursor::new(byte, text.len(), true), at, byte)
}

/// The scalar position of byte offset `to`, counted from the nearby `byte` (at scalar `at`) rather
/// than from the start of the text.
fn moved(text: &str, at: usize, byte: usize, to: usize) -> usize {
    if to >= byte {
        at + text[byte..to].chars().count()
    } else {
        at - text[to..byte].chars().count()
    }
}

/// The last grapheme boundary at or before `at`: where a caret that landed mid-character belongs.
#[must_use]
pub fn snap(text: &str, at: usize) -> usize {
    let (mut cursor, at, byte) = cursor(text, at);
    if byte >= text.len() || cursor.is_boundary(text, 0).unwrap_or(true) {
        return at;
    }
    match cursor.prev_boundary(text, 0) {
        Ok(Some(prev)) => moved(text, at, byte, prev),
        _ => 0,
    }
}

/// One character to the right; stays at the end.
#[must_use]
pub fn next_grapheme(text: &str, at: usize) -> usize {
    let (mut cursor, at, byte) = cursor(text, at);
    match cursor.next_boundary(text, 0) {
        Ok(Some(next)) => moved(text, at, byte, next),
        _ => at,
    }
}

/// One character to the left; stays at the start. From past the end, it goes to the end.
#[must_use]
pub fn previous_grapheme(text: &str, at: usize) -> usize {
    let asked = at;
    let (mut cursor, at, byte) = cursor(text, at);
    if asked > at {
        return at;
    }
    match cursor.prev_boundary(text, 0) {
        Ok(Some(prev)) => moved(text, at, byte, prev),
        _ => 0,
    }
}

/// The words in `text` as scalar ranges, in order and produced lazily, so a jump stops reading once
/// it has its word. Spaces and punctuation between them are not words.
fn words(text: &str) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
    text.split_word_bounds()
        .scan(0, |pos, segment| {
            let start = *pos;
            *pos += segment.chars().count();
            Some((start..*pos, segment))
        })
        .filter(|(_, segment)| segment.chars().any(char::is_alphanumeric))
        .map(|(range, _)| range)
}

/// Option/Ctrl+Right: to the end of the word the caret is in, or of the next word.
#[must_use]
pub fn word_right(text: &str, at: usize) -> usize {
    words(text)
        .find(|word| word.end > at)
        .map_or_else(|| text.chars().count(), |word| word.end)
}

/// Option/Ctrl+Left: to the start of the word the caret is in, or of the previous word.
#[must_use]
pub fn word_left(text: &str, at: usize) -> usize {
    words(text)
        .take_while(|word| word.start < at)
        .last()
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
