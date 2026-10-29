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
    /// Whether it is an input method's draft (drawn underlined, not yet in the page).
    pub draft: bool,
}

/// Splits a row's runs where the selection and the draft start and end, so each piece has one look.
pub(in crate::shell) fn pieces(
    spans: &[TextSpan],
    selected: Option<&Range<usize>>,
    draft: Option<&Range<usize>>,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let (mut chars, mut bytes) = (0, 0);
    let inside = |range: Option<&Range<usize>>, at: usize| range.is_some_and(|r| r.start <= at && at < r.end);
    for span in spans {
        let len = span.text.chars().count();
        let mut cuts: Vec<usize> = vec![0, len];
        for range in [selected, draft].into_iter().flatten() {
            for edge in [range.start, range.end] {
                if edge > chars && edge < chars + len {
                    cuts.push(edge - chars);
                }
            }
        }
        cuts.sort_unstable();
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
            let first = chars + from;
            out.push(Piece {
                bytes: bytes..bytes + width,
                marks: span.marks.clone(),
                selected: inside(selected, first),
                draft: inside(draft, first),
            });
            bytes += width;
        }
        chars += len;
    }
    out
}

/// A row's runs with an input method's draft shown in place of `covered` (the selection it will
/// replace, or an empty range at the caret). Returns the runs and where the draft sits, in characters.
pub(in crate::shell) fn with_draft(
    spans: &[TextSpan],
    covered: &Range<usize>,
    draft: &str,
) -> (Vec<TextSpan>, Range<usize>) {
    let mut out = Vec::new();
    let mut chars = 0;
    let mut placed = false;
    let draft_span = TextSpan {
        text: draft.to_owned(),
        marks: Marks::default(),
    };
    for span in spans {
        let len = span.text.chars().count();
        let (start, end) = (chars, chars + len);
        let keep_before: String = span
            .text
            .chars()
            .take(covered.start.saturating_sub(start).min(len))
            .collect();
        let keep_after: String = span
            .text
            .chars()
            .skip(covered.end.saturating_sub(start).min(len))
            .collect();
        if end <= covered.start {
            out.push(span.clone());
        } else if start >= covered.end {
            if !placed {
                out.push(draft_span.clone());
                placed = true;
            }
            out.push(span.clone());
        } else {
            if !keep_before.is_empty() {
                out.push(TextSpan {
                    text: keep_before,
                    marks: span.marks.clone(),
                });
            }
            if !placed {
                out.push(draft_span.clone());
                placed = true;
            }
            if !keep_after.is_empty() {
                out.push(TextSpan {
                    text: keep_after,
                    marks: span.marks.clone(),
                });
            }
        }
        chars = end;
    }
    if !placed {
        out.push(draft_span);
    }
    let len = draft.chars().count();
    (out, covered.start..covered.start + len)
}

/// The UTF-8 byte offset of character `chars` in `text` (the end if past it).
pub(in crate::shell) fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

#[cfg(test)]
mod tests;
