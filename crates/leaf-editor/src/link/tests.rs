//! Tests for following links (E07-S07).

use leaf_model::{Mark, PageCommand};

use super::openable_url;
use crate::block_actions::tests::{at, p, page};

#[test]
fn the_link_at_a_place_is_the_one_touching_it() {
    let (mut editor, ids) = page(&[p(0, "see docs here"), p(0, "plain")]);
    let block = ids[0];
    editor
        .doc
        .apply(PageCommand::SetMark {
            id: block,
            range: 4..8,
            mark: Mark::Link("https://example.com".into()),
        })
        .unwrap();
    let href = |editor: &crate::PageEditor, offset| editor.link_at(at(block, offset)).unwrap();
    assert_eq!(href(&editor, 3), None, "before it");
    for offset in 4..=8 {
        assert_eq!(
            href(&editor, offset).as_deref(),
            Some("https://example.com"),
            "at {offset}"
        );
    }
    assert_eq!(href(&editor, 9), None, "after it");
    assert_eq!(href(&editor, 99), None, "past the end");
    assert_eq!(editor.link_at(at(ids[1], 0)).unwrap(), None);
}

#[test]
fn web_and_mail_addresses_open_and_nothing_else_does() {
    assert_eq!(
        openable_url("https://example.com/a?b#c").as_deref(),
        Some("https://example.com/a?b#c")
    );
    assert_eq!(
        openable_url("  HTTP://Example.com ").as_deref(),
        Some("HTTP://Example.com")
    );
    assert_eq!(
        openable_url("mailto:me@example.com").as_deref(),
        Some("mailto:me@example.com")
    );
    assert_eq!(
        openable_url("example.com/page").as_deref(),
        Some("https://example.com/page")
    );
    assert_eq!(
        openable_url("www.example.co.uk").as_deref(),
        Some("https://www.example.co.uk")
    );
    for refused in [
        "",
        "   ",
        "https://",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "slack://open",
        "notes",
        "two words.com",
        ".com",
        "example.",
        "localhost:8080/x",
    ] {
        assert_eq!(openable_url(refused), None, "{refused:?}");
    }
}
