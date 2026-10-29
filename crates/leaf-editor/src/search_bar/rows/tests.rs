//! Tests for converting the index's character ranges to byte ranges.

#![allow(
    clippy::single_range_in_vec_init,
    reason = "lists holding one matched range are the data under test"
)]

use super::byte_ranges;
use crate::search_bar::Marked;

fn marked(text: &str, ranges: &[std::ops::Range<usize>]) -> Marked {
    Marked {
        text: text.to_owned(),
        ranges: ranges.to_vec(),
    }
}

#[test]
fn character_ranges_become_byte_ranges() {
    assert_eq!(byte_ranges(&marked("garden", &[0..3])), vec![0..3]);
    // "é" and "…" take more than one byte each.
    let text = "…café garden";
    let ranges = byte_ranges(&marked(text, &[1..5, 6..12]));
    assert_eq!(&text[ranges[0].clone()], "café");
    assert_eq!(&text[ranges[1].clone()], "garden");
}

#[test]
fn ranges_past_the_end_are_clipped_and_empty_ones_dropped() {
    assert_eq!(byte_ranges(&marked("abc", &[1..9])), vec![1..3]);
    assert_eq!(byte_ranges(&marked("abc", &[5..9, 2..2])), Vec::new());
}
