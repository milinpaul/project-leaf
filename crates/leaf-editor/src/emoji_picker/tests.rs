//! Tests for the page icon picker.

use super::{EmojiPicker, RESULTS_TITLE};

fn found(picker: &EmojiPicker) -> Vec<&'static str> {
    picker
        .sections()
        .iter()
        .flat_map(|section| section.emojis.iter().map(|choice| choice.emoji))
        .collect()
}

#[test]
fn with_nothing_typed_every_group_is_a_section() {
    let picker = EmojiPicker::new();
    let titles: Vec<&str> = picker.sections().iter().map(|s| s.title).collect();
    assert_eq!(
        titles,
        [
            "Smileys & emotion",
            "People & body",
            "Animals & nature",
            "Food & drink",
            "Travel & places",
            "Activities",
            "Objects",
            "Symbols",
            "Flags"
        ]
    );
    let total: usize = picker.sections().iter().map(|s| s.emojis.len()).sum();
    assert!(total > 1_500, "the full set, not a sample ({total})");
    assert_eq!(picker.first().map(|c| c.emoji), Some("😀"));
    assert_eq!(EmojiPicker::default().sections(), picker.sections());
}

#[test]
fn search_matches_names_and_shortcodes_best_first() {
    let mut picker = EmojiPicker::new();
    picker.set_query("rocket");
    assert_eq!(picker.sections()[0].title, RESULTS_TITLE);
    assert_eq!(found(&picker)[0], "🚀", "the whole name wins");

    picker.set_query("thumbs");
    assert!(found(&picker).contains(&"👍"));
    picker.set_query("+1");
    assert_eq!(found(&picker), ["👍"], "GitHub shortcodes work too");

    picker.set_query("food");
    let food = found(&picker);
    assert!(
        food.len() > 50 && food.contains(&"🍎"),
        "a group's name finds the whole group"
    );
    assert_eq!(
        food[0], "🥫",
        "but an emoji named for it comes first (canned food)"
    );

    picker.set_query("red heart");
    assert_eq!(found(&picker)[0], "❤️");
    picker.set_query("HEART red");
    assert!(found(&picker).contains(&"❤️"), "any order, any case");
}

#[test]
fn every_word_must_start_a_word() {
    let mut picker = EmojiPicker::new();
    picker.set_query("eart"); // inside "heart" and "earth", but starts no word
    assert!(!found(&picker).contains(&"❤️"));
    picker.set_query("zzzz");
    assert!(picker.sections().is_empty(), "nothing matches");
    assert_eq!(picker.first(), None);
    assert_eq!(picker.query(), "zzzz");
    picker.set_query("  ");
    assert_eq!(
        picker.sections().len(),
        9,
        "clearing the search shows everything again"
    );
}

#[test]
fn skin_tones_and_too_new_emoji_are_left_out() {
    let picker = EmojiPicker::new();
    let all = found(&picker);
    assert!(all.contains(&"👋"));
    assert!(!all.contains(&"👋🏽"), "skin-tone variants are not offered");
    assert!(
        !all.contains(&"🫠"),
        "melting face is Unicode 14, which older fonts can't draw"
    );
}

#[test]
fn random_picks_a_friendly_emoji_and_depends_only_on_the_seed() {
    assert_eq!(EmojiPicker::random(42), EmojiPicker::random(42));
    let picks: std::collections::HashSet<&str> =
        (0..50).map(|seed| EmojiPicker::random(seed).emoji).collect();
    assert!(picks.len() > 40, "different seeds give different emoji");
    let flags: std::collections::HashSet<&str> = EmojiPicker::new()
        .sections()
        .iter()
        .filter(|s| s.title == "Flags" || s.title == "Symbols")
        .flat_map(|s| s.emojis.iter().map(|c| c.emoji))
        .collect();
    assert!(
        (0..200).all(|seed| !flags.contains(EmojiPicker::random(seed).emoji)),
        "no flags or symbols"
    );
}
