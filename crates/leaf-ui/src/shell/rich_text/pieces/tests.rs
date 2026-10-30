//! Tests for selection highlights per row and splitting runs into pieces.

use leaf_core::BlockId;
use leaf_editor::{BlockRow, BlockStyle, Selection};
use leaf_model::{Caret, Marks, TextSpan};

use super::{Highlight, Piece, byte_of, highlights, pieces, with_draft};

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

#[test]
fn selected_cells_are_covered_whole() {
    let rows = vec![
        row(0, None),
        row(1, Some("a")),
        row(1, Some("b")),
        row(1, Some("c")),
    ];
    let lit = highlights(
        &rows,
        Some(&Selection::Cells {
            table: rows[0].id,
            cells: vec![rows[1].id, rows[3].id],
        }),
    );
    assert_eq!(lit.get(&rows[1].id), Some(&Highlight::Whole));
    assert_eq!(lit.get(&rows[3].id), Some(&Highlight::Whole));
    assert!(!lit.contains_key(&rows[2].id) && !lit.contains_key(&rows[0].id));
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
    let got = pieces(&spans, Some(&(3..8)), None);
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
        pieces(&spans, None, None).len(),
        2,
        "without a selection, one piece per run"
    );
    assert_eq!(pieces(&[], Some(&(0..0)), None), Vec::<Piece>::new());
}

#[test]
fn character_offsets_become_byte_offsets() {
    let text = "a👋🏽b";
    assert_eq!(byte_of(text, 0), 0);
    assert_eq!(byte_of(text, 1), 1);
    assert_eq!(byte_of(text, 3), 9);
    assert_eq!(byte_of(text, 99), text.len());
}

#[test]
fn a_draft_shows_in_place_of_what_it_covers_and_is_flagged() {
    let spans = vec![span("ab", false), span("cd", true)];
    // At the caret, inside a run: the run splits around the draft.
    let (with, at) = with_draft(&spans, &(1..1), "か");
    let texts: Vec<&str> = with.iter().map(|s| s.text.as_str()).collect();
    assert_eq!((texts, at.clone()), (vec!["a", "か", "b", "cd"], 1..2));
    let drawn = pieces(&with, None, Some(&at));
    assert_eq!(
        drawn.iter().map(|p| p.draft).collect::<Vec<_>>(),
        vec![false, true, false, false]
    );
    assert_eq!(drawn[1].bytes, 1..4, "か is three bytes");
    // Over a selection spanning two runs: the selected text is hidden.
    let (with, at) = with_draft(&spans, &(1..3), "XY");
    let texts: Vec<&str> = with.iter().map(|s| s.text.as_str()).collect();
    assert_eq!((texts, at), (vec!["a", "XY", "d"], 1..3));
    // At the very end, and on an empty block.
    let (with, _) = with_draft(&spans, &(4..4), "!");
    assert_eq!(with.last().map(|s| s.text.as_str()), Some("!"));
    let (with, at) = with_draft(&[], &(0..0), "क");
    assert_eq!((with.len(), at), (1, 0..1));
}
