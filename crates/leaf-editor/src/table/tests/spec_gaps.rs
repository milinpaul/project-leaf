//! Table rules from `PRODUCT_SPEC.md` "Editor behaviour" that the Phase 0 acceptance pass (E08-S03)
//! found only partly tested: moving inside a cell's text, the second press that deletes a selected
//! table, Backspace over selected cells, and the plain text of copied cells.

use leaf_core::BlockId;
use leaf_model::PageDoc;

use super::{Grid, Page, at, page_copy};
use crate::{Key, KeyOutcome, LineLayout, Motion, Selection, Vertical};

/// Like [`Grid`], but a block's text is split into lines at its line breaks, so a cell can have more
/// than one line. x is the offset within the line, plus the cell's column offset.
struct Lines<'a>(&'a PageDoc);

impl Lines<'_> {
    fn text(&self, block: BlockId) -> Vec<char> {
        self.0
            .plain_text(block)
            .unwrap()
            .unwrap_or_default()
            .chars()
            .collect()
    }

    /// Where each line starts, as character offsets.
    fn starts(&self, block: BlockId) -> Vec<usize> {
        let text = self.text(block);
        let mut starts = vec![0];
        starts.extend(
            text.iter()
                .enumerate()
                .filter(|(_, c)| **c == '\n')
                .map(|(i, _)| i + 1),
        );
        starts
    }
}

impl LineLayout for Lines<'_> {
    fn line_count(&self, block: BlockId) -> usize {
        self.starts(block).len()
    }
    fn line_of(&self, block: BlockId, offset: usize) -> usize {
        self.starts(block)
            .iter()
            .filter(|&&start| start <= offset)
            .count()
            - 1
    }
    fn offset_to_x(&self, block: BlockId, offset: usize) -> f32 {
        let start = self.starts(block)[self.line_of(block, offset)];
        #[expect(clippy::cast_precision_loss, reason = "small test offsets")]
        let x = Grid(self.0).left(block) + (offset - start) as f32;
        x
    }
    fn x_to_offset(&self, block: BlockId, line: usize, x: f32) -> usize {
        let starts = self.starts(block);
        let start = starts[line];
        let end = starts
            .get(line + 1)
            .map_or_else(|| self.text(block).len(), |next| next - 1);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test positions"
        )]
        let along = (x - Grid(self.0).left(block)).max(0.0) as usize;
        (start + along).min(end)
    }
}

#[test]
fn arrows_move_through_a_cells_text_before_leaving_it() {
    let mut p = Page::new(2, 2);
    let cell = p.cell(0, 0);
    p.put(cell, 2);
    p.editor.move_caret(Motion::Right).unwrap();
    assert_eq!(p.caret(), at(cell, 3), "one character, same cell");
    p.editor.move_caret(Motion::Left).unwrap();
    p.editor.move_caret(Motion::Left).unwrap();
    assert_eq!(p.caret(), at(cell, 1));

    // A cell with a line break: Up and Down go between its lines before leaving it.
    let tall = p.cell(0, 1);
    p.editor.set_text(tall, "ab\ncd").unwrap();
    let shape = page_copy(&p);
    p.put(tall, 1);
    p.editor.move_vertically(Vertical::Down, &Lines(&shape)).unwrap();
    assert_eq!(p.caret(), at(tall, 4), "the cell's second line, same column");
    p.editor.move_vertically(Vertical::Down, &Lines(&shape)).unwrap();
    assert_eq!(p.caret().block, p.cell(1, 1), "then the cell below");
    p.editor.move_vertically(Vertical::Up, &Lines(&shape)).unwrap();
    assert_eq!(p.caret().block, tall, "back up into the tall cell");
    assert_eq!(
        p.editor.doc().table_cells(p.table).unwrap()[0][1],
        tall,
        "nothing moved"
    );
}

#[test]
fn a_selected_table_goes_on_the_second_press_and_comes_back_in_one_undo() {
    for (start, key) in [(true, Key::Backspace), (false, Key::Delete)] {
        let mut p = Page::new(2, 2);
        // Backspace at the start of the block below; Delete at the end of the block above.
        let (block, offset) = if start { (p.below, 0) } else { (p.above, 5) };
        p.put(block, offset);
        assert_eq!(
            p.press(key),
            KeyOutcome::SelectBlock(p.table),
            "{key:?} selects first"
        );
        assert!(p.editor.doc().contains(p.table));
        p.press(key);
        assert!(!p.editor.doc().contains(p.table), "{key:?} again deletes it");
        assert_eq!(p.editor.doc().root_blocks(), [p.above, p.below]);
        assert!(p.editor.undo().unwrap());
        assert!(p.editor.doc().contains(p.table), "one undo brings it back");
        assert_eq!(p.grid(), [["r0c0", "r0c1"], ["r1c0", "r1c1"]], "with its cells");
    }
}

#[test]
fn backspace_over_selected_cells_empties_them_in_one_step() {
    let mut p = Page::new(2, 3);
    p.editor
        .select_between(at(p.cell(0, 1), 2), at(p.cell(1, 2), 1))
        .unwrap();
    assert!(matches!(
        p.editor.selection().unwrap(),
        Some(Selection::Cells { .. })
    ));
    p.press(Key::Backspace);
    assert_eq!(
        p.grid(),
        [["r0c0", "", ""], ["r1c0", "", ""]],
        "exactly those cells empty; the table keeps its shape"
    );
    assert!(p.editor.undo().unwrap());
    assert_eq!(
        p.grid(),
        [["r0c0", "r0c1", "r0c2"], ["r1c0", "r1c1", "r1c2"]],
        "one undo step"
    );
}

#[test]
fn selected_cells_copy_to_other_apps_as_tab_separated_rows() {
    let mut p = Page::new(3, 3);
    p.editor
        .select_between(at(p.cell(1, 0), 0), at(p.cell(2, 1), 0))
        .unwrap();
    let copied = p.editor.copy().unwrap().unwrap();
    assert_eq!(copied.to_plain_text(), "r1c0\tr1c1\nr2c0\tr2c1");
}
