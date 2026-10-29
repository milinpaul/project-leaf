//! The plain, testable half of drawing a block's text: which part of each row the selection covers,
//! and how a row's formatted runs split into pieces that each draw with one style.

use std::collections::HashMap;
use std::ops::Range;

use leaf_core::BlockId;
use leaf_editor::{BlockRow, Selection};
use leaf_model::{Marks, TextSpan};

/// How much of a row the selection covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::shell) enum Highlight {
    /// These characters (Unicode scalars) of its text.
    Text(Range<usize>),
    /// The whole block: it is selected as a block, or sits inside one that is.
    Whole,
}

/// The highlight of every row the selection touches.
pub(in crate::shell) fn highlights(
    rows: &[BlockRow],
    selection: Option<&Selection>,
) -> HashMap<BlockId, Highlight> {
    let mut out = HashMap::new();
    match selection {
        None => {}
        Some(Selection::Text { start, end }) => {
            let at = |b: BlockId| rows.iter().position(|r| r.id == b);
            let (Some(first), Some(last)) = (at(start.block), at(end.block)) else {
                return out;
            };
            for row in &rows[first..=last] {
                let len = row.text.as_ref().map_or(0, |t| t.chars().count());
                let from = if row.id == start.block { start.at } else { 0 };
                let to = if row.id == end.block { end.at } else { len };
                // Blocks without text in between (dividers, images) are covered whole.
                let highlight = if row.text.is_none() {
                    Highlight::Whole
                } else {
                    Highlight::Text(from..to)
                };
                out.insert(row.id, highlight);
            }
        }
        Some(Selection::Blocks(blocks)) => {
            // A selected block covers everything nested in it: the rows after it that are deeper.
            let mut inside: Option<usize> = None;
            for row in rows {
                if inside.is_some_and(|depth| row.depth <= depth) {
                    inside = None;
                }
                if inside.is_none() && blocks.contains(&row.id) {
                    inside = Some(row.depth);
                }
                if inside.is_some() {
                    out.insert(row.id, Highlight::Whole);
                }
            }
        }
    }
    out
}

/// A run of text that draws with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::shell) struct Piece {
    /// UTF-8 byte range in the row's text, as text layout counts.
    pub bytes: Range<usize>,
    /// Its formatting.
    pub marks: Marks,
    /// Whether it is inside the selection.
    pub selected: bool,
}

/// Splits a row's runs where the selection starts and ends, so each piece has one look.
pub(in crate::shell) fn pieces(spans: &[TextSpan], selected: Option<&Range<usize>>) -> Vec<Piece> {
    let mut out = Vec::new();
    let (mut chars, mut bytes) = (0, 0);
    for span in spans {
        let mut cuts: Vec<usize> = vec![0];
        let len = span.text.chars().count();
        if let Some(sel) = selected {
            for edge in [sel.start, sel.end] {
                if edge > chars && edge < chars + len {
                    cuts.push(edge - chars);
                }
            }
        }
        cuts.push(len);
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            let width: usize = span
                .text
                .chars()
                .skip(from)
                .take(to - from)
                .map(char::len_utf8)
                .sum();
            let middle = chars + from;
            out.push(Piece {
                bytes: bytes..bytes + width,
                marks: span.marks.clone(),
                selected: selected.is_some_and(|sel| sel.start <= middle && middle < sel.end),
            });
            bytes += width;
        }
        chars += len;
    }
    out
}

/// The UTF-8 byte offset of character `chars` in `text` (the end if past it).
pub(in crate::shell) fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

#[cfg(test)]
mod tests;
