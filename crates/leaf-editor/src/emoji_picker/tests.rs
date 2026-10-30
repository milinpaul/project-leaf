//! Tests for the page icon picker.

use super::{EmojiPicker, GridMove, RESULTS_TITLE};

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

#[test]
fn the_first_emoji_is_highlighted_and_follows_the_search() {
    let mut picker = EmojiPicker::new();
    assert_eq!(picker.highlighted(), Some((0, 0)));
    assert_eq!(picker.highlighted_choice(), picker.first());
    picker.set_query("rocket");
    assert_eq!(picker.highlighted_choice().map(|c| c.emoji), Some("🚀"));
    picker.set_query("zzzzqqq");
    assert_eq!(picker.highlighted(), None, "nothing shown, nothing highlighted");
    picker.move_highlight(GridMove::Down, 10);
    assert_eq!(picker.highlighted(), None);
}

#[test]
fn arrows_move_through_the_grid_and_across_sections() {
    let mut picker = EmojiPicker::new();
    let len0 = picker.sections()[0].emojis.len();
    picker.move_highlight(GridMove::Left, 10);
    assert_eq!(picker.highlighted(), Some((0, 0)), "nothing before the first");
    picker.move_highlight(GridMove::Up, 10);
    assert_eq!(picker.highlighted(), Some((0, 0)), "nothing above the first row");
    picker.move_highlight(GridMove::Right, 10);
    picker.move_highlight(GridMove::Right, 10);
    assert_eq!(picker.highlighted(), Some((0, 2)));
    picker.move_highlight(GridMove::Down, 10);
    assert_eq!(picker.highlighted(), Some((0, 12)), "one row down, same column");
    picker.move_highlight(GridMove::Up, 10);
    assert_eq!(picker.highlighted(), Some((0, 2)));

    // From the last row of a section down into the next one's first row, keeping the column.
    let last_row = (len0 - 1) / 10 * 10;
    picker.highlight(0, last_row + 1);
    picker.move_highlight(GridMove::Down, 10);
    assert_eq!(picker.highlighted(), Some((1, 1)));
    // And back up into the last row, same column if it reaches that far.
    picker.move_highlight(GridMove::Up, 10);
    assert_eq!(picker.highlighted(), Some((0, (last_row + 1).min(len0 - 1))));

    // Left from a section's first emoji goes to the previous section's last; Right comes back.
    picker.highlight(1, 0);
    picker.move_highlight(GridMove::Left, 10);
    assert_eq!(picker.highlighted(), Some((0, len0 - 1)));
    picker.move_highlight(GridMove::Right, 10);
    assert_eq!(picker.highlighted(), Some((1, 0)));
}

#[test]
fn down_on_a_row_above_a_short_last_row_lands_on_its_last_emoji() {
    let mut picker = EmojiPicker::new();
    let (section, len) = picker
        .sections()
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.emojis.len()))
        .find(|&(_, len)| len % 10 != 0 && len > 10)
        .unwrap();
    let last_row = (len - 1) / 10 * 10;
    picker.highlight(section, last_row - 1);
    picker.move_highlight(GridMove::Down, 10);
    assert_eq!(picker.highlighted(), Some((section, len - 1)));
}

#[test]
fn the_mouse_highlights_only_real_emoji() {
    let mut picker = EmojiPicker::new();
    picker.highlight(1, 3);
    assert_eq!(picker.highlighted(), Some((1, 3)));
    picker.highlight(99, 0);
    picker.highlight(1, 100_000);
    assert_eq!(picker.highlighted(), Some((1, 3)));
}
