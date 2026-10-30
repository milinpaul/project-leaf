//! Tests for editing in tables (E16-S02): one per table rule in `PRODUCT_SPEC.md` "Editor
//! behaviour", then the guards that keep other features from splitting, merging or converting cells.

use leaf_core::BlockId;
use leaf_model::{BlockKind, Caret, PageCommand, PageDoc, PageOutcome};

use crate::{
    BlockAction, Fragment, FragmentBlock, Key, KeyOutcome, LineLayout, Motion, PageEditor, RowBox, Selection,
    Vertical,
};

/// A page with a paragraph "above", a `rows` × `columns` table whose cells read "r0c0", "r0c1", ...,
/// and a paragraph "below". Loaded from bytes so building it can't be undone.
struct Page {
    editor: PageEditor,
    above: BlockId,
    table: BlockId,
    below: BlockId,
}

impl Page {
    fn new(rows: usize, columns: usize) -> Self {
        Self::with_text(rows, columns, true)
    }

    fn with_text(rows: usize, columns: usize, labelled: bool) -> Self {
        let mut doc = PageDoc::new();
        let block = |doc: &mut PageDoc, command| match doc.apply(command) {
            Ok(PageOutcome::BlockCreated(id)) => id,
            other => panic!("expected a block, got {other:?}"),
        };
        let above = block(
            &mut doc,
            PageCommand::InsertBlock {
                parent: None,
                index: 0,
                kind: BlockKind::Paragraph,
                text: "above".into(),
            },
        );
        let table = block(
            &mut doc,
            PageCommand::InsertTable {
                parent: None,
                index: 1,
                rows,
                columns,
                header_row: true,
            },
        );
        let below = block(
            &mut doc,
            PageCommand::InsertBlock {
                parent: None,
                index: 2,
                kind: BlockKind::Paragraph,
                text: "below".into(),
            },
        );
        if labelled {
            for (r, row) in doc.table_cells(table).unwrap().into_iter().enumerate() {
                for (c, cell) in row.into_iter().enumerate() {
                    doc.apply(PageCommand::InsertText {
                        id: cell,
                        pos: 0,
                        text: format!("r{r}c{c}"),
                    })
                    .unwrap();
                }
            }
        }
        let mut doc = PageDoc::from_bytes(&doc.to_bytes().unwrap()).unwrap();
        doc.set_undo_merge_interval(0);
        Self {
            editor: PageEditor::new(doc),
            above,
            table,
            below,
        }
    }

    fn cell(&self, row: usize, column: usize) -> BlockId {
        self.editor.doc().table_cells(self.table).unwrap()[row][column]
    }

    fn text(&self, block: BlockId) -> String {
        self.editor.doc().plain_text(block).unwrap().unwrap_or_default()
    }

    fn grid(&self) -> Vec<Vec<String>> {
        self.editor
            .doc()
            .table_cells(self.table)
            .unwrap()
            .into_iter()
            .map(|row| row.into_iter().map(|c| self.text(c)).collect())
            .collect()
    }

    fn put(&mut self, block: BlockId, at: usize) {
        self.editor.set_caret(Caret { block, at }).unwrap();
    }

    fn press(&mut self, key: Key) -> KeyOutcome {
        self.editor.press(key).unwrap()
    }

    fn caret(&mut self) -> Caret {
        self.editor.caret().unwrap().unwrap()
    }
}

fn at(block: BlockId, at: usize) -> Caret {
    Caret { block, at }
}

/// Every block on one line; a cell's text starts at 20 × its column, other blocks at 0.
struct Grid<'a>(&'a PageDoc);

impl Grid<'_> {
    fn left(&self, block: BlockId) -> f32 {
        let Ok(Some(row)) = self.0.parent(block) else {
            return 0.0;
        };
        if self.0.kind(block).unwrap() != BlockKind::TableCell {
            return 0.0;
        }
        let column = self.0.children(row).unwrap().iter().position(|&c| c == block);
        #[expect(clippy::cast_precision_loss, reason = "small test columns")]
        let left = column.unwrap_or(0) as f32 * 20.0;
        left
    }
}

impl LineLayout for Grid<'_> {
    fn line_count(&self, _: BlockId) -> usize {
        1
    }
    fn line_of(&self, _: BlockId, _: usize) -> usize {
        0
    }
    fn offset_to_x(&self, block: BlockId, offset: usize) -> f32 {
        #[expect(clippy::cast_precision_loss, reason = "small test offsets")]
        let x = self.left(block) + offset as f32;
        x
    }
    fn x_to_offset(&self, block: BlockId, _: usize, x: f32) -> usize {
        let len = self.0.plain_text(block).unwrap().map_or(0, |t| t.chars().count());
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test positions"
        )]
        let offset = (x - self.left(block)).max(0.0) as usize;
        offset.min(len)
    }
}

#[test]
fn tab_and_shift_tab_move_between_cells_and_tab_in_the_last_cell_adds_a_row() {
    let mut p = Page::new(2, 2);
    p.put(p.cell(0, 0), 1);
    p.press(Key::Tab);
    assert_eq!(
        p.caret(),
        at(p.cell(0, 1), 4),
        "next cell, caret at the end of its text"
    );
    p.press(Key::Tab);
    assert_eq!(p.caret(), at(p.cell(1, 0), 4), "the next row's first cell");
    p.press(Key::ShiftTab);
    assert_eq!(p.caret(), at(p.cell(0, 1), 4));
    p.put(p.cell(0, 0), 0);
    p.press(Key::ShiftTab);
    assert_eq!(p.caret(), at(p.cell(0, 0), 0), "nothing before the first cell");

    p.put(p.cell(1, 1), 2);
    p.press(Key::Tab);
    assert_eq!(p.grid().len(), 3, "a row was added");
    assert_eq!(p.caret(), at(p.cell(2, 0), 0), "in its first cell");
    assert!(p.editor.undo().unwrap());
    assert_eq!(p.grid().len(), 2, "one undo takes the row away");
    assert_eq!(p.caret(), at(p.cell(1, 1), 2));
}

#[test]
fn left_and_right_cross_into_the_neighbouring_cell_at_the_edge() {
    let mut p = Page::new(2, 2);
    p.put(p.cell(0, 0), 4);
    p.editor.move_caret(Motion::Right).unwrap();
    assert_eq!(p.caret(), at(p.cell(0, 1), 0));
    p.editor.move_caret(Motion::Left).unwrap();
    assert_eq!(p.caret(), at(p.cell(0, 0), 4));
    p.put(p.cell(0, 0), 0);
    p.editor.move_caret(Motion::Left).unwrap();
    assert_eq!(p.caret(), at(p.above, 5), "out of the table, before it");
    p.put(p.cell(1, 1), 4);
    p.editor.move_caret(Motion::Right).unwrap();
    assert_eq!(p.caret(), at(p.below, 0), "out of the table, after it");
}

#[test]
fn up_and_down_keep_to_the_column_and_leave_at_the_top_and_bottom_rows() {
    let mut p = Page::new(3, 3);
    p.put(p.cell(0, 1), 2);
    let shape = page_copy(&p);
    let down = |p: &mut Page| {
        p.editor.move_vertically(Vertical::Down, &Grid(&shape)).unwrap();
    };
    down(&mut p);
    assert_eq!(p.caret().block, p.cell(1, 1), "the cell below, same column");
    down(&mut p);
    assert_eq!(p.caret().block, p.cell(2, 1));
    down(&mut p);
    assert_eq!(p.caret().block, p.below, "past the bottom row");

    p.put(p.cell(0, 2), 0);
    p.editor.move_vertically(Vertical::Up, &Grid(&shape)).unwrap();
    assert_eq!(p.caret().block, p.above, "past the top row");
}

/// A copy of the page for the layout to read its shape from (block IDs are the same), so the editor
/// can move the caret while the layout is borrowed.
fn page_copy(p: &Page) -> PageDoc {
    PageDoc::from_bytes(&p.editor.doc().to_bytes().unwrap()).unwrap()
}

#[test]
fn coming_into_a_table_from_above_or_below_picks_the_column_under_the_caret() {
    let mut p = Page::new(2, 3);
    let shape = page_copy(&p);
    // "above" is at x 0..5; x 4 is in the first column (0..20).
    p.put(p.above, 4);
    p.editor.move_vertically(Vertical::Down, &Grid(&shape)).unwrap();
    assert_eq!(p.caret().block, p.cell(0, 0));

    // Aim at x 45 from below: the third column starts at 40.
    let below = p.below;
    p.editor.set_text(below, "below the table, a long line").unwrap();
    let shape = page_copy(&p);
    p.put(below, 25);
    p.editor.move_vertically(Vertical::Up, &Grid(&shape)).unwrap();
    assert_eq!(p.caret().block, p.cell(1, 1), "x 25 is in the second column");
}

#[test]
fn enter_types_a_line_break_in_a_cell() {
    let mut p = Page::new(1, 2);
    let cell = p.cell(0, 1);
    p.put(cell, 2);
    p.press(Key::Enter);
    assert_eq!(p.text(cell), "r0\nc1");
    assert_eq!(p.caret(), at(cell, 3));
    assert_eq!(p.grid().len(), 1, "no block was added");
    p.editor.select(cell, 0..2).unwrap();
    p.press(Key::Enter);
    assert_eq!(p.text(cell), "\n\nc1", "over a selection, the break replaces it");
    p.editor.set_caret(at(cell, 0)).unwrap();
    p.press(Key::ModEnter);
    assert_eq!(p.text(cell), "\n\n\nc1");
}

#[test]
fn backspace_and_delete_never_merge_cells() {
    let mut p = Page::new(2, 2);
    p.put(p.cell(1, 0), 0);
    assert_eq!(p.press(Key::Backspace), KeyOutcome::Caret(at(p.cell(1, 0), 0)));
    p.put(p.cell(0, 0), 4);
    assert_eq!(p.press(Key::Delete), KeyOutcome::Caret(at(p.cell(0, 0), 4)));
    p.press(Key::Backspace);
    assert_eq!(
        p.text(p.cell(0, 0)),
        "r0c",
        "inside the text, Backspace deletes as usual"
    );
    assert_eq!(p.grid().len(), 2);
    assert_eq!(p.grid()[0].len(), 2, "nothing merged");
}

#[test]
fn backspace_in_an_empty_table_deletes_it_in_one_step() {
    let mut p = Page::with_text(2, 2, false);
    p.put(p.cell(1, 1), 0);
    p.press(Key::Backspace);
    assert!(!p.editor.doc().contains(p.table), "the empty table is gone");
    assert_eq!(p.caret(), at(p.above, 5), "caret at the end of the block above");
    assert!(p.editor.undo().unwrap());
    assert!(p.editor.doc().contains(p.table), "one undo brings it back");
    assert_eq!(p.grid(), [["", ""], ["", ""]]);
}

#[test]
fn a_selected_table_is_deleted_by_backspace() {
    let mut p = Page::new(2, 2);
    p.put(p.below, 0);
    assert_eq!(
        p.press(Key::Backspace),
        KeyOutcome::SelectBlock(p.table),
        "Backspace below the table selects it"
    );
    assert_eq!(
        p.editor.selection().unwrap(),
        Some(Selection::Blocks(vec![p.table]))
    );
    p.press(Key::Backspace);
    assert!(!p.editor.doc().contains(p.table));
    assert_eq!(p.editor.doc().root_blocks(), [p.above, p.below]);

    let mut p = Page::new(1, 1);
    p.put(p.above, 5);
    assert_eq!(
        p.press(Key::Delete),
        KeyOutcome::SelectBlock(p.table),
        "Delete above it too"
    );
}

#[test]
fn a_selection_across_cells_covers_the_cells_between_them() {
    let mut p = Page::new(3, 3);
    p.editor
        .select_between(at(p.cell(0, 2), 3), at(p.cell(1, 1), 1))
        .unwrap();
    let Some(Selection::Cells { table, cells }) = p.editor.selection().unwrap() else {
        panic!("expected cells");
    };
    assert_eq!(table, p.table);
    assert_eq!(cells, [p.cell(0, 1), p.cell(0, 2), p.cell(1, 1), p.cell(1, 2)]);

    p.press(Key::Delete);
    assert_eq!(
        p.grid(),
        [["r0c0", "", ""], ["r1c0", "", ""], ["r2c0", "r2c1", "r2c2"]],
        "the cells empty, the table keeps its shape"
    );
    assert_eq!(p.caret(), at(p.cell(0, 1), 0));
    assert!(p.editor.undo().unwrap());
    assert_eq!(p.grid()[0], ["r0c0", "r0c1", "r0c2"], "one undo step");
}

#[test]
fn typing_over_selected_cells_empties_them_and_types_in_the_first() {
    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.cell(0, 0), 1), at(p.cell(1, 0), 2))
        .unwrap();
    p.editor.insert_text("x").unwrap();
    assert_eq!(p.grid(), [["x", "r0c1"], ["", "r1c1"]]);
}

#[test]
fn selected_cells_copy_as_one_paragraph_each_and_cut_empties_them() {
    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.cell(0, 0), 0), at(p.cell(1, 1), 0))
        .unwrap();
    let Some(Fragment::Blocks(blocks)) = p.editor.copy().unwrap() else {
        panic!("expected blocks");
    };
    let texts: Vec<String> = blocks
        .iter()
        .map(|b| {
            assert_eq!(b.kind, BlockKind::Paragraph);
            b.text.iter().map(|s| s.text.as_str()).collect()
        })
        .collect();
    assert_eq!(texts, ["r0c0", "r0c1", "r1c0", "r1c1"]);
    p.editor.cut().unwrap();
    assert_eq!(p.grid(), [["", ""], ["", ""]]);
}

#[test]
fn formatting_selected_cells_formats_all_their_text() {
    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.cell(0, 0), 2), at(p.cell(0, 1), 1))
        .unwrap();
    p.editor.toggle_mark(leaf_model::MarkKind::Bold).unwrap();
    for cell in [p.cell(0, 0), p.cell(0, 1)] {
        let spans = p.editor.doc().text(cell).unwrap().unwrap();
        assert!(spans.len() == 1 && spans[0].marks.bold, "{spans:?}");
    }
    let spans = p.editor.doc().text(p.cell(1, 0)).unwrap().unwrap();
    assert!(!spans[0].marks.bold, "only the selected cells");
}

#[test]
fn a_selection_reaching_into_a_table_takes_the_table_whole() {
    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.above, 2), at(p.cell(0, 1), 2))
        .unwrap();
    assert!(matches!(
        p.editor.selection().unwrap(),
        Some(Selection::Text { .. })
    ));
    p.press(Key::Backspace);
    assert!(!p.editor.doc().contains(p.table));
    assert_eq!(p.text(p.above), "ab");
    assert_eq!(p.caret(), at(p.above, 2));
    assert_eq!(p.editor.doc().root_blocks(), [p.above, p.below], "nothing joined");

    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.cell(1, 0), 1), at(p.below, 2))
        .unwrap();
    p.press(Key::Delete);
    assert!(!p.editor.doc().contains(p.table));
    assert_eq!(p.text(p.below), "low");
    assert_eq!(p.caret(), at(p.below, 0));
    assert!(p.editor.undo().unwrap());
    assert_eq!(p.grid().len(), 2, "one undo step");
}

#[test]
fn pasting_blocks_into_a_cell_puts_their_text_in_one_line_each() {
    let mut p = Page::new(1, 1);
    let cell = p.cell(0, 0);
    p.put(cell, 4);
    let para = |text: &str| FragmentBlock {
        kind: BlockKind::Paragraph,
        text: vec![leaf_model::TextSpan {
            text: text.into(),
            marks: leaf_model::Marks::default(),
        }],
        children: Vec::new(),
    };
    p.editor
        .paste(&Fragment::Blocks(vec![para(" one"), para("two")]))
        .unwrap();
    assert_eq!(p.text(cell), "r0c0 one\ntwo");
    assert_eq!(p.grid().len(), 1, "no blocks were added");
    p.editor.paste_text("- three\n- four").unwrap();
    assert_eq!(p.text(cell), "r0c0 one\ntwothree\nfour");
}

#[test]
fn a_copied_table_pastes_as_a_paragraph_per_cell_for_now() {
    let mut p = Page::new(2, 2);
    p.editor.select_blocks(&[p.table]).unwrap();
    let fragment = p.editor.copy().unwrap().unwrap();
    p.put(p.below, 5);
    p.editor.paste(&fragment).unwrap();
    let texts: Vec<String> = p
        .editor
        .doc()
        .root_blocks()
        .into_iter()
        .filter_map(|b| p.editor.doc().plain_text(b).unwrap())
        .collect();
    assert_eq!(texts, ["above", "belowr0c0", "r0c1", "r1c0", "r1c1"]);
}

#[test]
fn shortcuts_and_the_slash_menu_stay_out_of_cells() {
    let mut p = Page::with_text(1, 1, false);
    let cell = p.cell(0, 0);
    p.put(cell, 0);
    p.editor.insert_text("# ").unwrap();
    assert_eq!(p.editor.doc().kind(cell).unwrap(), BlockKind::TableCell);
    assert_eq!(p.text(cell), "# ");
    p.editor.insert_text("/").unwrap();
    assert!(p.editor.slash_menu().unwrap().is_none());
}

#[test]
fn a_picture_asked_for_in_a_cell_goes_below_the_table() {
    let mut p = Page::with_text(1, 1, false);
    p.put(p.cell(0, 0), 0);
    let caret = p
        .editor
        .insert_image(leaf_core::AssetId::of(b"png"))
        .unwrap()
        .unwrap();
    let roots = p.editor.doc().root_blocks();
    assert_eq!(roots[2], caret.block, "right after the table");
    assert!(matches!(
        p.editor.doc().kind(caret.block).unwrap(),
        BlockKind::Image { .. }
    ));
    assert_eq!(p.editor.doc().kind(p.cell(0, 0)).unwrap(), BlockKind::TableCell);
}

#[test]
fn block_actions_and_nesting_from_a_cell_act_on_the_whole_table() {
    let mut p = Page::new(1, 2);
    let cell = p.cell(0, 0);
    assert_eq!(p.editor.action_targets(cell).unwrap(), [p.table]);
    p.editor
        .select_between(at(p.cell(0, 0), 0), at(p.cell(0, 1), 1))
        .unwrap();
    assert_eq!(p.editor.action_targets(cell).unwrap(), [p.table]);

    p.editor
        .block_action(cell, BlockAction::TurnInto(BlockKind::Quote))
        .unwrap();
    assert!(
        matches!(p.editor.doc().kind(p.table).unwrap(), BlockKind::Table { .. }),
        "a table stays a table"
    );
    p.editor.block_action(cell, BlockAction::Duplicate).unwrap();
    assert_eq!(
        p.editor.doc().root_blocks().len(),
        4,
        "the table was copied whole"
    );

    // Tab over a selection from the table into the paragraph below nests the whole table with it.
    let mut p = Page::new(1, 1);
    p.editor
        .select_between(at(p.cell(0, 0), 2), at(p.below, 2))
        .unwrap();
    p.press(Key::Tab);
    assert_eq!(
        p.editor.doc().children(p.above).unwrap(),
        [p.table, p.below],
        "both nested under the block above, the table whole"
    );
}

#[test]
fn a_block_dropped_among_cells_lands_after_the_table() {
    let mut p = Page::new(2, 1);
    let rows: Vec<RowBox> = p
        .editor
        .view()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, row)| {
            #[expect(clippy::cast_precision_loss, reason = "a few test rows")]
            let top = i as f32 * 10.0;
            RowBox {
                block: row.id,
                top,
                bottom: top + 10.0,
            }
        })
        .collect();
    // Rows: above, table, r0c0, r1c0, below. Pointer on the first cell's lower half.
    let drop = p
        .editor
        .drop_target(&[p.above], &rows, 0.0, 28.0, 24.0)
        .unwrap()
        .unwrap();
    assert_eq!(drop.below, Some(p.table));
    assert_eq!(drop.parent, None);
    assert!(p.editor.drop_blocks(&[p.above], &drop).unwrap());
    assert_eq!(p.editor.doc().root_blocks(), [p.table, p.above, p.below]);
}

#[test]
fn undo_puts_back_a_cell_selection() {
    let mut p = Page::new(2, 2);
    p.editor
        .select_between(at(p.cell(0, 0), 0), at(p.cell(1, 1), 0))
        .unwrap();
    let before = p.editor.selection().unwrap();
    assert!(matches!(before, Some(Selection::Cells { .. })));
    p.press(Key::Delete);
    assert!(p.editor.undo().unwrap());
    assert_eq!(p.editor.selection().unwrap(), before);
}
