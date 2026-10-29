//! Tests for selection highlights per row and splitting runs into pieces.

use leaf_core::BlockId;
use leaf_editor::{BlockRow, BlockStyle, Selection};
use leaf_model::{Caret, Marks, TextSpan};

use super::{Highlight, Piece, byte_of, highlights, pieces};

fn row(depth: usize, text: Option<&str>) -> BlockRow {
    BlockRow {
        id: BlockId::new(),
        depth,
        style: BlockStyle::Paragraph,
        text: text.map(str::to_owned),
        spans: None,
    }
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

#[test]
fn a_text_selection_covers_its_ends_partly_and_the_rows_between_whole() {
    let rows = vec![
        row(0, Some("abcd")),
        row(0, None),
        row(1, Some("middle")),
        row(0, Some("wxyz")),
        row(0, Some("after")),
    ];
    let selection = Selection::Text {
        start: at(rows[0].id, 1),
        end: at(rows[3].id, 2),
    };
    let lit = highlights(&rows, Some(&selection));
    assert_eq!(lit[&rows[0].id], Highlight::Text(1..4));
    assert_eq!(lit[&rows[1].id], Highlight::Whole, "a divider in between");
    assert_eq!(lit[&rows[2].id], Highlight::Text(0..6));
    assert_eq!(lit[&rows[3].id], Highlight::Text(0..2));
    assert!(!lit.contains_key(&rows[4].id));
    assert!(highlights(&rows, None).is_empty());

    let inside = Selection::Text {
        start: at(rows[2].id, 1),
        end: at(rows[2].id, 3),
    };
    assert_eq!(highlights(&rows, Some(&inside)).len(), 1);
    let gone = Selection::Text {
        start: at(BlockId::new(), 0),
        end: at(rows[0].id, 1),
    };
    assert!(
        highlights(&rows, Some(&gone)).is_empty(),
        "an end not on the page highlights nothing"
    );
}

#[test]
fn selected_blocks_cover_everything_nested_in_them() {
    let rows = vec![
        row(0, Some("a")),
        row(1, Some("a.1")),
        row(2, Some("a.1.1")),
        row(0, Some("b")),
        row(1, Some("b.1")),
    ];
    let lit = highlights(&rows, Some(&Selection::Blocks(vec![rows[0].id, rows[4].id])));
    let covered: Vec<bool> = rows.iter().map(|r| lit.contains_key(&r.id)).collect();
    assert_eq!(covered, vec![true, true, true, false, true]);
}

fn span(text: &str, bold: bool) -> TextSpan {
    TextSpan {
        text: text.into(),
        marks: Marks {
            bold,
            ..Marks::default()
        },
    }
}

#[test]
fn pieces_split_runs_at_the_selection_and_count_bytes() {
    // "héllo" (é is two bytes) plain, then "wörld" bold; select "lo wö".
    let spans = vec![span("héllo ", false), span("wörld", true)];
    let got = pieces(&spans, Some(&(3..8)));
    let summary: Vec<(std::ops::Range<usize>, bool, bool)> = got
        .iter()
        .map(|p: &Piece| (p.bytes.clone(), p.marks.bold, p.selected))
        .collect();
    assert_eq!(
        summary,
        vec![
            (0..4, false, false),
            (4..7, false, true),
            (7..10, true, true),
            (10..13, true, false)
        ]
    );
    assert_eq!(
        pieces(&spans, None).len(),
        2,
        "without a selection, one piece per run"
    );
    assert_eq!(pieces(&[], Some(&(0..0))), Vec::<Piece>::new());
}

#[test]
fn character_offsets_become_byte_offsets() {
    let text = "a👋🏽b";
    assert_eq!(byte_of(text, 0), 0);
    assert_eq!(byte_of(text, 1), 1);
    assert_eq!(byte_of(text, 3), 9);
    assert_eq!(byte_of(text, 99), text.len());
}
