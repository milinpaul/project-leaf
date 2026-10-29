use super::{CLOSE, OPEN, highlighted};

#[test]
fn markers_become_character_ranges() {
    let marked = format!("café {OPEN}crème{CLOSE} and {OPEN}tea{CLOSE}");
    let hl = highlighted(&marked);
    assert_eq!(hl.text, "café crème and tea");
    assert_eq!(hl.ranges, [5..10, 15..18]);
}

#[test]
fn an_unmatched_close_marker_is_ignored() {
    let hl = highlighted(&format!("a{CLOSE}b"));
    assert_eq!((hl.text.as_str(), hl.ranges.len()), ("ab", 0));
}
