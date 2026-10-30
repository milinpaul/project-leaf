//! Tests for caret movement inside a block's text, with the scripts and emoji that break naive code.

use proptest::prelude::*;

use super::{line_end, line_start, next_grapheme, previous_grapheme, snap, word_left, word_right};

/// Every position the caret stops at, pressing `step` from `start` until it stops moving.
fn walk(text: &str, start: usize, step: fn(&str, usize) -> usize) -> Vec<usize> {
    let mut stops = vec![start];
    loop {
        let at = *stops.last().unwrap_or(&start);
        let next = step(text, at);
        if next == at {
            return stops;
        }
        stops.push(next);
    }
}

fn len(text: &str) -> usize {
    text.chars().count()
}

#[test]
fn plain_text_moves_one_character_at_a_time() {
    assert_eq!(walk("abc", 0, next_grapheme), vec![0, 1, 2, 3]);
    assert_eq!(walk("abc", 3, previous_grapheme), vec![3, 2, 1, 0]);
    assert_eq!(walk("", 0, next_grapheme), vec![0]);
    assert_eq!(walk("", 0, previous_grapheme), vec![0]);
}

#[test]
fn emoji_zwj_sequences_flags_and_skin_tones_are_one_step() {
    let family = "👨\u{200d}👩\u{200d}👧\u{200d}👦"; // 7 scalars
    let flag = "🇮🇳"; // 2 regional indicators
    let thumbs = "👍🏽"; // emoji + skin tone
    let text = format!("a{family}{flag}{thumbs}b");
    assert_eq!(walk(&text, 0, next_grapheme), vec![0, 1, 8, 10, 12, 13]);
    assert_eq!(walk(&text, 13, previous_grapheme), vec![13, 12, 10, 8, 1, 0]);
}

#[test]
fn combining_marks_stay_with_their_letter() {
    let text = "e\u{301}a"; // é written as e + combining acute
    assert_eq!(walk(text, 0, next_grapheme), vec![0, 2, 3]);
    assert_eq!(walk(text, 3, previous_grapheme), vec![3, 2, 0]);
}

#[test]
fn devanagari_syllables_and_conjuncts_are_one_step() {
    // न | म | स्ते : the vowel sign and the conjunct joined by virama are part of their cluster.
    let namaste = "नमस्ते";
    assert_eq!(len(namaste), 6);
    assert_eq!(walk(namaste, 0, next_grapheme), vec![0, 1, 2, 6]);
    // क्षि (ksha + i) is one cluster under Unicode 15.1's conjunct rule.
    assert_eq!(walk("क्षि", 0, next_grapheme), vec![0, 4]);
    assert_eq!(walk("क्षि", 4, previous_grapheme), vec![4, 0]);
}

#[test]
fn a_caret_inside_a_cluster_snaps_back_to_its_start() {
    let family = "👨\u{200d}👩\u{200d}👧";
    assert_eq!(snap(family, 3), 0);
    assert_eq!(snap(family, 5), 5);
    assert_eq!(snap("ab", 9), 2, "past the end snaps to the end");
    assert_eq!(next_grapheme(family, 2), 5);
    assert_eq!(previous_grapheme(family, 2), 0);
}

#[test]
fn word_jumps_skip_spaces_and_punctuation() {
    let text = "Hello,  world! foo_bar 42";
    assert_eq!(walk(text, 0, word_right), vec![0, 5, 13, 22, 25]);
    assert_eq!(walk(text, 25, word_left), vec![25, 23, 15, 8, 0]);
    assert_eq!(word_right(text, 2), 5, "from inside a word, to its end");
    assert_eq!(word_left(text, 2), 0, "from inside a word, to its start");
    assert_eq!(word_right("  ...  ", 0), 7, "no words: to the end");
    assert_eq!(word_left("  ...  ", 7), 0, "no words: to the start");
}

#[test]
fn word_jumps_handle_emoji_and_devanagari() {
    let text = "नमस्ते दुनिया 👋 hi";
    // नमस्ते = 6, space, दुनिया = 6, space, 👋, space, hi
    assert_eq!(walk(text, 0, word_right), vec![0, 6, 13, 18]);
    assert_eq!(walk(text, 18, word_left), vec![18, 16, 7, 0]);
}

#[test]
fn home_and_end_go_to_the_ends_of_the_current_line() {
    let text = "one\ntwo three\n\nend";
    assert_eq!(line_start(text, 6), 4);
    assert_eq!(line_end(text, 6), 13);
    assert_eq!(line_start(text, 4), 4);
    assert_eq!(line_end(text, 13), 13);
    assert_eq!(line_start(text, 14), 14, "an empty line");
    assert_eq!(line_end(text, 14), 14);
    assert_eq!(line_start(text, 2), 0);
    assert_eq!(line_end(text, 16), 18);
    assert_eq!((line_start("", 0), line_end("", 0)), (0, 0));
}

fn tricky_text() -> impl Strategy<Value = String> {
    let pieces = prop::sample::select(vec![
        "a",
        " ",
        ",",
        "\n",
        "é",
        "e\u{301}",
        "👍🏽",
        "🇮🇳",
        "👨\u{200d}👩\u{200d}👧",
        "न",
        "स्ते",
        "क्षि",
        "中",
        "_",
        "9",
    ]);
    prop::collection::vec(pieces, 0..12).prop_map(|parts| parts.concat())
}

proptest! {
    /// Every motion lands on a grapheme boundary, inside the text, and Left undoes Right.
    #[test]
    fn every_motion_lands_on_a_boundary(text in tricky_text(), seed in 0..64_usize) {
        let n = len(&text);
        let at = snap(&text, seed % (n + 1));
        for step in [next_grapheme, previous_grapheme, word_right, word_left, line_start, line_end] {
            let to = step(&text, at);
            prop_assert!(to <= n);
            prop_assert_eq!(snap(&text, to), to, "{:?} from {} went to {}", text, at, to);
        }
        if at < n {
            prop_assert_eq!(previous_grapheme(&text, next_grapheme(&text, at)), at);
        }
        prop_assert!(word_right(&text, at) >= at && word_left(&text, at) <= at);
    }
}

/// The motions as they were before E17-S09, segmenting the whole text from its start: the local
/// versions must agree with them everywhere.
mod whole_text {
    use unicode_segmentation::UnicodeSegmentation;

    fn boundaries(text: &str) -> Vec<usize> {
        std::iter::once(0)
            .chain(text.graphemes(true).scan(0, |pos, g| {
                *pos += g.chars().count();
                Some(*pos)
            }))
            .collect()
    }

    pub fn snap(text: &str, at: usize) -> usize {
        boundaries(text)
            .into_iter()
            .take_while(|&b| b <= at)
            .last()
            .unwrap_or(0)
    }

    pub fn next(text: &str, at: usize) -> usize {
        boundaries(text)
            .into_iter()
            .find(|&b| b > at)
            .unwrap_or_else(|| text.chars().count())
    }

    pub fn previous(text: &str, at: usize) -> usize {
        boundaries(text)
            .into_iter()
            .take_while(|&b| b < at)
            .last()
            .unwrap_or(0)
    }

    fn words(text: &str) -> Vec<std::ops::Range<usize>> {
        let (mut pos, mut out) = (0, Vec::new());
        for segment in text.split_word_bounds() {
            let len = segment.chars().count();
            if segment.chars().any(char::is_alphanumeric) {
                out.push(pos..pos + len);
            }
            pos += len;
        }
        out
    }

    pub fn word_right(text: &str, at: usize) -> usize {
        words(text)
            .into_iter()
            .find(|w| w.end > at)
            .map_or_else(|| text.chars().count(), |w| w.end)
    }

    pub fn word_left(text: &str, at: usize) -> usize {
        words(text)
            .into_iter()
            .rev()
            .find(|w| w.start < at)
            .map_or(0, |w| w.start)
    }
}

/// Like `tricky_text`, longer and with lone regional indicators, digits with points and apostrophes too.
fn trickier_text() -> impl Strategy<Value = String> {
    let pieces = prop::sample::select(vec![
        "a",
        "Z",
        " ",
        "  ",
        ".",
        ",",
        "'",
        "3.14",
        "e\u{301}",
        "👨\u{200d}👩\u{200d}👧",
        "🇮",
        "🇳",
        "🇮🇳",
        "👍🏽",
        "क्ष",
        "नमस्ते",
        "東京",
        "\n",
        "-",
        "don't",
    ]);
    prop::collection::vec(pieces, 0..40).prop_map(|parts| parts.concat())
}

proptest! {
    #[test]
    fn local_motions_agree_with_whole_text_segmentation(text in trickier_text(), seed in 0usize..1000) {
        let len = text.chars().count();
        let at = if len == 0 { 0 } else { seed % (len + 2) };
        prop_assert_eq!(snap(&text, at), whole_text::snap(&text, at), "snap {} in {:?}", at, text);
        prop_assert_eq!(next_grapheme(&text, at), whole_text::next(&text, at), "next {} in {:?}", at, text);
        prop_assert_eq!(previous_grapheme(&text, at), whole_text::previous(&text, at), "previous {} in {:?}", at, text);
        prop_assert_eq!(word_right(&text, at), whole_text::word_right(&text, at), "word right {} in {:?}", at, text);
        prop_assert_eq!(word_left(&text, at), whole_text::word_left(&text, at), "word left {} in {:?}", at, text);
    }
}
