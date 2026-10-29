//! Tests for Up and Down (E04-S03), against a fake monospace layout.

use std::collections::HashMap;

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};
use unicode_segmentation::UnicodeSegmentation;

use super::{LineLayout, Vertical};
use crate::{Motion, PageEditor};

/// Columns per wrapped line in the fake layout.
const WIDTH: usize = 10;
/// Indentation per nesting level, in columns.
const INDENT: usize = 4;

/// A monospace layout: every grapheme is one column wide, lines wrap after [`WIDTH`] graphemes
/// (no word wrap), newlines start a new line, and nested blocks are indented.
struct FakeLayout {
    /// Per block: its indentation and its visual lines, each the caret positions on it in order.
    /// A wrapped line leaves out its wrap point, which belongs to the next line.
    blocks: HashMap<BlockId, (usize, Vec<Vec<usize>>)>,
}

impl FakeLayout {
    fn of(editor: &PageEditor) -> Self {
        let blocks = editor
            .rows()
            .unwrap()
            .into_iter()
            .map(|row| {
                let text = row.text.unwrap_or_default();
                (row.id, (row.depth * INDENT, lines(&text)))
            })
            .collect();
        Self { blocks }
    }

    fn block(&self, block: BlockId) -> &(usize, Vec<Vec<usize>>) {
        &self.blocks[&block]
    }
}

fn lines(text: &str) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for hard in text.split('\n') {
        let mut stops = vec![start];
        for grapheme in hard.graphemes(true) {
            stops.push(stops[stops.len() - 1] + grapheme.chars().count());
        }
        let end = stops[stops.len() - 1];
        let mut chunks: Vec<Vec<usize>> = stops.chunks(WIDTH).map(<[usize]>::to_vec).collect();
        // A line exactly WIDTH long would leave its end alone on a line of its own.
        if chunks.len() > 1 && chunks[chunks.len() - 1].len() == 1 && stops.len() % WIDTH == 1 {
            chunks.pop();
            if let Some(last) = chunks.last_mut() {
                last.push(end);
            }
        }
        out.extend(chunks);
        start = end + 1;
    }
    out
}

impl LineLayout for FakeLayout {
    fn line_count(&self, block: BlockId) -> usize {
        self.block(block).1.len()
    }

    fn line_of(&self, block: BlockId, offset: usize) -> usize {
        let lines = &self.block(block).1;
        lines
            .iter()
            .position(|line| line.contains(&offset))
            .unwrap_or(lines.len() - 1)
    }

    fn offset_to_x(&self, block: BlockId, offset: usize) -> f32 {
        let (indent, lines) = self.block(block);
        let line = &lines[self.line_of(block, offset)];
        let column = line.iter().position(|&o| o == offset).unwrap_or(0);
        x_of(indent + column)
    }

    fn x_to_offset(&self, block: BlockId, line: usize, x: f32) -> usize {
        let (indent, lines) = self.block(block);
        let stops = &lines[line];
        // The stop whose column is closest to x.
        let closest = (0..stops.len())
            .min_by(|&a, &b| {
                let distance = |column| (x_of(indent + column) - x).abs();
                distance(a).total_cmp(&distance(b))
            })
            .unwrap_or(0);
        stops[closest]
    }
}

/// The x of a column: one unit per column.
fn x_of(column: usize) -> f32 {
    f32::from(u16::try_from(column).unwrap_or(u16::MAX))
}

fn page(blocks: &[(usize, BlockKind, &str)]) -> (PageEditor, Vec<BlockId>) {
    let mut doc = PageDoc::new();
    let mut ids: Vec<BlockId> = Vec::new();
    let mut parents: Vec<BlockId> = Vec::new();
    for (depth, kind, text) in blocks {
        parents.truncate(*depth);
        let parent = parents.last().copied();
        let index = match parent {
            Some(p) => doc.children(p).unwrap().len(),
            None => doc.root_blocks().len(),
        };
        let Ok(PageOutcome::BlockCreated(id)) = doc.apply(PageCommand::InsertBlock {
            parent,
            index,
            kind: kind.clone(),
            text: (*text).into(),
        }) else {
            panic!("insert failed");
        };
        parents.push(id);
        ids.push(id);
    }
    (PageEditor::new(doc), ids)
}

fn para(text: &str) -> (usize, BlockKind, &str) {
    (0, BlockKind::Paragraph, text)
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// Presses Up or Down in turn and records where the caret goes.
fn presses(editor: &mut PageEditor, keys: &[Vertical]) -> Vec<Caret> {
    let layout = FakeLayout::of(editor);
    keys.iter()
        .map(|&key| editor.move_vertically(key, &layout).unwrap().unwrap())
        .collect()
}

use Vertical::{Down, Up};

#[test]
fn the_fake_layout_wraps_and_breaks_lines_as_described() {
    assert_eq!(lines(""), vec![vec![0]]);
    assert_eq!(lines("abc"), vec![vec![0, 1, 2, 3]]);
    assert_eq!(lines("0123456789"), vec![(0..=10).collect::<Vec<_>>()]);
    assert_eq!(
        lines("0123456789ab"),
        vec![(0..10).collect::<Vec<_>>(), vec![10, 11, 12]]
    );
    assert_eq!(lines("a\nbc"), vec![vec![0, 1], vec![2, 3, 4]]);
}

#[test]
fn nothing_moves_before_the_caret_is_placed() {
    let (mut editor, _) = page(&[para("abc")]);
    let layout = FakeLayout::of(&editor);
    assert_eq!(editor.move_vertically(Down, &layout).unwrap(), None);
}

#[test]
fn up_and_down_move_between_wrapped_lines_of_one_block() {
    // Lines: [0..10) "aaaa bbbb ", [10..20) "cccc dddd ", [20..24] "eeee".
    let (mut editor, ids) = page(&[para("aaaa bbbb cccc dddd eeee")]);
    let b = ids[0];
    editor.set_caret(at(b, 3)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down, Down, Up, Up]),
        vec![at(b, 13), at(b, 23), at(b, 13), at(b, 3)]
    );
}

#[test]
fn the_caret_keeps_its_x_through_short_lines_and_blocks() {
    let (mut editor, ids) = page(&[para("long line!"), para("ab"), para("another one")]);
    editor.set_caret(at(ids[0], 8)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down, Down, Up, Up]),
        vec![at(ids[1], 2), at(ids[2], 8), at(ids[1], 2), at(ids[0], 8)]
    );
}

#[test]
fn moving_into_a_wrapped_block_lands_on_its_first_or_last_line() {
    let (mut editor, ids) = page(&[para("top"), para("0123456789abcdefghij-xyz"), para("bottom")]);
    editor.set_caret(at(ids[0], 2)).unwrap();
    assert_eq!(presses(&mut editor, &[Down]), vec![at(ids[1], 2)]);
    editor.set_caret(at(ids[2], 1)).unwrap();
    assert_eq!(presses(&mut editor, &[Up]), vec![at(ids[1], 21)]);
}

#[test]
fn indentation_counts_so_the_caret_stays_in_the_same_screen_column() {
    let (mut editor, ids) = page(&[
        (0, BlockKind::BulletedList, "a parent"),
        (1, BlockKind::BulletedList, "child"),
    ]);
    editor.set_caret(at(ids[0], 6)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down, Up]),
        vec![at(ids[1], 6 - INDENT), at(ids[0], 6)]
    );
    editor.set_caret(at(ids[0], 1)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down]),
        vec![at(ids[1], 0)],
        "left of the indented text: its start"
    );
}

#[test]
fn dividers_and_images_are_passed_over() {
    let (mut editor, ids) = page(&[para("above"), (0, BlockKind::Divider, ""), para("below")]);
    editor.set_caret(at(ids[0], 3)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down, Up]),
        vec![at(ids[2], 3), at(ids[0], 3)]
    );
}

#[test]
fn the_page_edges_go_to_the_start_and_end_and_keep_the_x() {
    let (mut editor, ids) = page(&[para("first"), para("last line")]);
    editor.set_caret(at(ids[0], 3)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Up, Down, Down, Down, Up]),
        vec![
            at(ids[0], 0),
            at(ids[1], 3),
            at(ids[1], 9),
            at(ids[1], 9),
            at(ids[0], 3)
        ]
    );
}

#[test]
fn code_lines_are_lines_too() {
    let (mut editor, ids) = page(&[(
        0,
        BlockKind::Code {
            language: String::new(),
        },
        "fn a() {\n    b();\n}",
    )]);
    let code = ids[0];
    editor.set_caret(at(code, 6)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down, Down, Up]),
        vec![at(code, 15), at(code, 19), at(code, 15)],
        "the last line is short, then the column comes back"
    );
}

#[test]
fn emoji_and_devanagari_count_as_one_column_and_are_never_split() {
    let family = "👨\u{200d}👩\u{200d}👧"; // 5 scalars, 1 column
    let first = format!("{family}{family}ab");
    let (mut editor, ids) = page(&[para(&first), para("नमस्ते"), para("xyz")]);
    editor.set_caret(at(ids[0], 10)).unwrap(); // column 2, after both families
    assert_eq!(
        presses(&mut editor, &[Down, Down]),
        vec![at(ids[1], 2), at(ids[2], 2)],
        "नमस्ते is three columns: न म स्ते"
    );
    editor.set_caret(at(ids[2], 1)).unwrap();
    assert_eq!(
        presses(&mut editor, &[Up, Up]),
        vec![at(ids[1], 1), at(ids[0], 5)]
    );
}

#[test]
fn any_other_caret_move_forgets_the_remembered_x() {
    let (mut editor, ids) = page(&[para("long line!"), para("ab"), para("another one")]);
    editor.set_caret(at(ids[0], 8)).unwrap();
    presses(&mut editor, &[Down]);
    editor.move_caret(Motion::Left).unwrap();
    assert_eq!(
        presses(&mut editor, &[Down]),
        vec![at(ids[2], 1)],
        "aims for the new column"
    );
    editor.set_caret(at(ids[0], 8)).unwrap();
    presses(&mut editor, &[Down]);
    editor.set_caret(at(ids[1], 0)).unwrap();
    assert_eq!(presses(&mut editor, &[Down]), vec![at(ids[2], 0)]);
}

#[test]
fn the_remembered_x_survives_edits_elsewhere() {
    let (mut editor, ids) = page(&[para("long line!"), para("ab"), para("another one")]);
    editor.set_caret(at(ids[0], 8)).unwrap();
    presses(&mut editor, &[Down]);
    editor.set_text(ids[2], "another one!").unwrap();
    assert_eq!(presses(&mut editor, &[Down]), vec![at(ids[2], 8)]);
}
