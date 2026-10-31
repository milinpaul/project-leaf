//! Tests for the web bookmark card's view model (E15-S04): which parts show, and the domain.

use leaf_core::AssetId;
use leaf_model::WebPreview;

use super::{WebBookmarkCard, domain_of};

/// The card as it reads on screen, one part per line, for snapshots.
fn picture(card: &WebBookmarkCard) -> String {
    let asset =
        |id: Option<AssetId>| id.map_or_else(|| "none".to_owned(), |id| id.to_string()[..8].to_owned());
    format!(
        "title:       {}\ndescription: {}\nfooter:      [icon {}] {}\npicture:     {}\nopens:       {}\nfetched: {}, fetching: {}",
        card.title,
        card.description.as_deref().unwrap_or("(none)"),
        asset(card.favicon),
        card.domain,
        asset(card.image),
        card.address,
        card.fetched,
        card.fetching,
    )
}

const LEAF: &str = "https://www.leaf.app/blog/local-first?ref=home";

fn full() -> WebPreview {
    WebPreview {
        title: Some("Local-first,   at last".into()),
        description: Some(
            "Why Leaf keeps your pages on your own computer,\nand how sync will work later.".into(),
        ),
        image: Some(AssetId::of(b"cover")),
        favicon: Some(AssetId::of(b"icon")),
        fetched_at: Some(1_793_000_000_000),
    }
}

#[test]
fn a_fetched_card_shows_every_part() {
    insta::assert_snapshot!(picture(&WebBookmarkCard::new(LEAF, &full(), false)));
}

#[test]
fn before_a_fetch_the_card_shows_the_address_and_domain() {
    insta::assert_snapshot!(picture(&WebBookmarkCard::new(LEAF, &WebPreview::default(), true)));
}

#[test]
fn missing_parts_collapse() {
    let only_title = WebPreview {
        title: Some("Leaf".into()),
        description: Some("   ".into()),
        fetched_at: Some(1),
        ..WebPreview::default()
    };
    insta::assert_snapshot!(picture(&WebBookmarkCard::new(LEAF, &only_title, false)));
    // Fetched but the page had nothing: still the address as the title.
    let nothing = WebPreview {
        fetched_at: Some(1),
        ..WebPreview::default()
    };
    let card = WebBookmarkCard::new(LEAF, &nothing, false);
    assert_eq!(card.title, LEAF);
    assert!(card.fetched && card.description.is_none() && card.image.is_none());
}

#[test]
fn long_titles_and_descriptions_are_cut() {
    let long = WebPreview {
        title: Some("t".repeat(1000)),
        description: Some("d".repeat(1000)),
        ..WebPreview::default()
    };
    let card = WebBookmarkCard::new(LEAF, &long, false);
    assert_eq!(card.title.chars().count(), 300);
    assert_eq!(card.description.map(|d| d.chars().count()), Some(500));
}

#[test]
fn the_domain_is_the_site_as_people_say_it() {
    for (url, domain) in [
        ("https://www.Leaf.app:8443/blog", "leaf.app"),
        ("http://leaf.app", "leaf.app"),
        ("https://docs.leaf.app/a?b#c", "docs.leaf.app"),
        ("https://me@leaf.app/", "leaf.app"),
        ("https://[2606:4700::1111]:443/x", "[2606:4700::1111]"),
        ("https://例え.jp/パス", "例え.jp"),
    ] {
        assert_eq!(domain_of(url), domain, "{url}");
    }
}
