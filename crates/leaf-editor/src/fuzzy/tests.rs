//! Tests for the quick switcher's fuzzy matcher.

use super::fuzzy_match;

fn score(text: &str, query: &str) -> u32 {
    fuzzy_match(text, query).map_or(0, |m| m.score)
}

/// The matched parts of `text`, joined with `|`.
fn matched(text: &str, query: &str) -> String {
    fuzzy_match(text, query)
        .unwrap()
        .ranges
        .into_iter()
        .map(|range| &text[range])
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn better_kinds_of_match_rank_higher() {
    let q = "notes";
    let ranked = [
        score("Notes", q),             // the whole title
        score("Notes from Monday", q), // its start
        score("Design notes", q),      // a word inside it
        score("Footnotes", q),         // inside a word
        score("No tea, lots", q),      // letters in order
    ];
    assert!(ranked.windows(2).all(|pair| pair[0] > pair[1]), "{ranked:?}");
}

#[test]
fn case_is_ignored_and_misses_are_none() {
    assert!(fuzzy_match("Release CHECKLIST", "checklist").is_some());
    assert!(fuzzy_match("Release checklist", "CHECK").is_some());
    assert_eq!(fuzzy_match("Garden plan", "zebra"), None);
    assert_eq!(
        fuzzy_match("Garden", "gardens"),
        None,
        "every letter must be there"
    );
}

#[test]
fn every_word_must_match_in_any_order() {
    assert_eq!(matched("Design notes", "notes design"), "Design|notes");
    assert_eq!(fuzzy_match("Design notes", "notes recipe"), None);
    assert_eq!(
        fuzzy_match("Anything", "   ").map(|m| m.score),
        Some(0),
        "blank query matches all"
    );
}

#[test]
fn matched_letters_are_reported_for_highlighting() {
    assert_eq!(matched("Release checklist", "check"), "check");
    assert_eq!(matched("Design notes", "dsgn"), "D|s|gn");
    assert_eq!(
        matched("Café menu", "café"),
        "Café",
        "byte ranges respect multi-byte letters"
    );
}

#[test]
fn among_equal_kinds_shorter_and_earlier_wins() {
    assert!(score("Plan", "pl") > score("Planning the garden", "pl"));
    assert!(score("A plan", "plan") > score("A long plan", "plan"));
    assert!(score("Disgust", "dsg") > score("Dosage", "dsg"), "fewer gaps win");
    assert!(
        score("Dark sky gazing", "dsg") > score("Disgust", "dsg"),
        "initials of words count for more than closeness"
    );
}
