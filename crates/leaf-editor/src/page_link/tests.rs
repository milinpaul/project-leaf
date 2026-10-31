//! Tests for drawing page links: labels, and the way between page and drawn positions.

use leaf_core::PageId;
use leaf_model::{Marks, PAGE_LINK, TextSpan};

use super::{LinkState, LinkTarget, LinkedText};

fn plain(text: &str) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks::default(),
    }
}

fn link(page: PageId) -> TextSpan {
    TextSpan {
        text: PAGE_LINK.to_string(),
        marks: Marks {
            page: Some(page),
            ..Marks::default()
        },
    }
}

fn japan(_: PageId) -> LinkTarget {
    LinkTarget {
        title: "Japan".into(),
        icon: Some("🗾".into()),
        state: LinkState::Live,
    }
}

#[test]
fn labels_show_the_icon_and_title_or_where_the_page_went() {
    assert_eq!(japan(PageId::new()).label(), "🗾 Japan");
    let untitled = LinkTarget {
        title: " ".into(),
        icon: None,
        state: LinkState::Live,
    };
    assert_eq!(untitled.label(), "Untitled");
    let trashed = LinkTarget {
        state: LinkState::Trashed,
        ..japan(PageId::new())
    };
    assert_eq!(trashed.label(), "Japan (in trash)");
    assert_eq!(trashed.name(), "Japan");
    assert_eq!(LinkTarget::missing().label(), "Deleted page");
    assert_eq!(LinkTarget::missing().name(), "Deleted page");
    assert_eq!(untitled.name(), "Untitled");
}

#[test]
fn text_without_links_is_drawn_as_it_is() {
    let spans = vec![plain("héllo")];
    let drawn = LinkedText::new(&spans, japan);
    assert_eq!(drawn.spans, spans);
    assert!(!drawn.has_links());
    for at in 0..=5 {
        assert_eq!(drawn.to_drawn(at), at);
        assert_eq!(drawn.to_page(at), at);
    }
}

#[test]
fn a_link_is_drawn_as_its_label_and_positions_map_both_ways() {
    let page = PageId::new();
    // "a" + link + "b": page positions 0..=3; drawn "a🗾 Japanb", the label 7 characters at 1..8.
    let drawn = LinkedText::new(&[plain("a"), link(page), plain("b")], japan);
    assert_eq!(drawn.text(), "a🗾 Japanb");
    assert_eq!(
        (0..=3).map(|at| drawn.to_drawn(at)).collect::<Vec<_>>(),
        vec![0, 1, 8, 9]
    );
    assert_eq!(drawn.links()[0].range, 1..8);
    assert_eq!(drawn.links()[0].page, page);
    // Inside the label, the nearer end of the link; the middle goes to its start.
    assert_eq!(
        (0..=9).map(|d| drawn.to_page(d)).collect::<Vec<_>>(),
        vec![0, 1, 1, 1, 1, 2, 2, 2, 2, 3]
    );
    assert_eq!(drawn.link_at(1).map(|l| l.page), Some(page));
    assert_eq!(drawn.link_at(7).map(|l| l.page), Some(page));
    assert_eq!(drawn.link_at(8), None);
    assert_eq!(drawn.link_at(0), None);
    assert_eq!(drawn.range_to_drawn(&(0..2)), 0..8);
    assert_eq!(drawn.to_drawn(99), 9, "past the end: the end");
}

#[test]
fn neighbouring_links_and_marks_keep_their_own_runs() {
    let (first, second) = (PageId::new(), PageId::new());
    let mut bold = link(first);
    bold.marks.bold = true;
    let drawn = LinkedText::new(&[bold, link(second)], |page| LinkTarget {
        title: if page == first { "A".into() } else { "B".into() },
        icon: None,
        state: LinkState::Live,
    });
    assert_eq!(drawn.text(), "AB");
    assert!(drawn.spans[0].marks.bold);
    assert_eq!(drawn.spans[1].marks.page, Some(second));
    assert_eq!(
        drawn.links().iter().map(|l| l.range.clone()).collect::<Vec<_>>(),
        vec![0..1, 1..2]
    );
}
